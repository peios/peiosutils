// `sd audit <path> <principal>:<perms>:<success|failure|both>` — append a
// system-audit ACE to the SACL. Supports --recursive and --if.

use crate::cmd::dacl::{AclKind, filter_acl, read_acl_builder, write_acl};
use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::flags::{self, TargetKind};
use crate::perms;
use crate::principal;
use crate::target::PathTarget;
use crate::walk;
use clap::ArgMatches;
use libp_sd::{AceBuilder, Condition, Sid, sddl};
use libp_sd::consts::{
    ACE_FLAG_FAILED_ACCESS, ACE_FLAG_SUCCESSFUL_ACCESS, ACE_TYPE_SYSTEM_AUDIT,
    ACE_TYPE_SYSTEM_AUDIT_CALLBACK,
};
use serde_json::json;

struct ParsedArgs {
    specs: Vec<(Sid, u32, u8)>, // (sid, mask, audit-bits)
    flags_override: Option<u8>,
    replace: bool,
    condition: Option<Condition>,
}

pub fn run(matches: &ArgMatches) -> Result<()> {
    let primary = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);
    let recursive = walk::parse_recursive(matches);

    let raw_specs: Vec<String> = matches
        .get_many::<String>("specs")
        .map(|it| it.cloned().collect())
        .unwrap_or_default();
    if raw_specs.is_empty() {
        return Err(Error::Usage("no PRINCIPAL:PERMS:KIND specs given".into()));
    }
    let mut specs = Vec::with_capacity(raw_specs.len());
    for spec in &raw_specs {
        specs.push(parse_audit_spec(spec)?);
    }
    let condition = match matches.get_one::<String>("if") {
        Some(s) => Some(
            sddl::parse_condition(s).map_err(|e| Error::Invalid(format!("--if: {e}")))?,
        ),
        None => None,
    };
    let args = ParsedArgs {
        specs,
        flags_override: match matches.get_one::<String>("flags") {
            Some(s) => Some(flags::parse(s)?),
            None => None,
        },
        replace: matches.get_flag("replace"),
        condition,
    };

    let targets = walk::walk_paths(&primary, recursive)?;
    let mut applied = 0usize;
    let mut errors: Vec<(String, Error)> = Vec::new();
    for target in &targets {
        match apply_one(target, &args) {
            Ok(n) => applied += n,
            Err(e) => errors.push((target.path.clone(), e)),
        }
    }

    match mode {
        OutputMode::Human => {
            println!(
                "{}: {} audit ACE(s) added across {} target(s)",
                primary.path,
                applied,
                targets.len()
            );
            for (p, e) in &errors {
                eprintln!("  {p}: {e}");
            }
        }
        OutputMode::Json => {
            let err_entries: Vec<_> = errors
                .iter()
                .map(|(p, e)| json!({ "path": p, "error": e.to_string() }))
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "path": primary.path,
                    "targets": targets.len(),
                    "added": applied,
                    "replace": args.replace,
                    "errors": err_entries,
                }))
                .unwrap()
            );
        }
    }
    if !errors.is_empty() {
        return Err(Error::Usage(format!("{} target(s) failed", errors.len())));
    }
    Ok(())
}

fn apply_one(target: &PathTarget, args: &ParsedArgs) -> Result<usize> {
    let target_kind = TargetKind::from_path(&target.path)
        .map_err(|e| Error::NotFound(format!("{}: {e}", target.path)))?;
    let base_flags = args
        .flags_override
        .unwrap_or_else(|| flags::default_for(target_kind));

    let new_sacl = if args.replace {
        let target_principals: Vec<Sid> = args.specs.iter().map(|(s, _, _)| s.clone()).collect();
        let (mut b, _dropped) = filter_acl(target, AclKind::Sacl, |ace| {
            if ace.ace_type != ACE_TYPE_SYSTEM_AUDIT && ace.ace_type != ACE_TYPE_SYSTEM_AUDIT_CALLBACK {
                return true;
            }
            if let Some((_, sid)) = ace.as_mask_sid() {
                let owned = sid.to_owned();
                !target_principals.contains(&owned)
            } else {
                true
            }
        })?;
        for (sid, mask, audit_bits) in &args.specs {
            let flags = base_flags | *audit_bits;
            b = b.ace(build_audit_ace(sid.clone(), *mask, flags, args.condition.as_ref())?);
        }
        b
    } else {
        let mut b = read_acl_builder(target, AclKind::Sacl)?;
        for (sid, mask, audit_bits) in &args.specs {
            let flags = base_flags | *audit_bits;
            b = b.ace(build_audit_ace(sid.clone(), *mask, flags, args.condition.as_ref())?);
        }
        b
    };

    write_acl(target, AclKind::Sacl, new_sacl)?;
    Ok(args.specs.len())
}

fn build_audit_ace(
    sid: Sid,
    mask: u32,
    flags: u8,
    condition: Option<&Condition>,
) -> Result<AceBuilder> {
    let b = match condition {
        None => AceBuilder::audit(sid, mask),
        Some(c) => AceBuilder::audit_callback(sid, mask, c)
            .map_err(|e| Error::Invalid(format!("conditional audit ACE: {e}")))?,
    };
    Ok(b.flags(flags))
}

fn parse_audit_spec(spec: &str) -> Result<(Sid, u32, u8)> {
    let parts: Vec<&str> = spec.rsplitn(3, ':').collect();
    if parts.len() != 3 {
        return Err(Error::Usage(format!(
            "audit spec `{spec}` needs PRINCIPAL:PERMS:KIND"
        )));
    }
    let kind_str = parts[0];
    let perms_str = parts[1];
    let principal_str = parts[2];

    let sid = principal::parse(principal_str)?;
    let mask = perms::parse(perms_str)?;
    let audit_bits = match kind_str.to_ascii_lowercase().as_str() {
        "success" | "s" => ACE_FLAG_SUCCESSFUL_ACCESS,
        "failure" | "fail" | "f" => ACE_FLAG_FAILED_ACCESS,
        "both" | "all" => ACE_FLAG_SUCCESSFUL_ACCESS | ACE_FLAG_FAILED_ACCESS,
        other => {
            return Err(Error::Usage(format!(
                "audit kind `{other}` (use success|failure|both)"
            )));
        }
    };
    Ok((sid, mask, audit_bits))
}
