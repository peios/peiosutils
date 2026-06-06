// `sd allow` and `sd deny` share the same surface — only the ACE kind
// differs. Implemented together. Supports --recursive and --if.

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
use libp_sd::consts::{ACE_TYPE_ACCESS_ALLOWED, ACE_TYPE_ACCESS_DENIED};
use serde_json::json;

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Allow,
    Deny,
}

impl Kind {
    fn ace_type(self) -> u8 {
        match self {
            Kind::Allow => ACE_TYPE_ACCESS_ALLOWED,
            Kind::Deny => ACE_TYPE_ACCESS_DENIED,
        }
    }
    fn verb(self) -> &'static str {
        match self {
            Kind::Allow => "allow",
            Kind::Deny => "deny",
        }
    }
    fn callback_ace_type(self) -> u8 {
        use libp_sd::consts::{ACE_TYPE_ACCESS_ALLOWED_CALLBACK, ACE_TYPE_ACCESS_DENIED_CALLBACK};
        match self {
            Kind::Allow => ACE_TYPE_ACCESS_ALLOWED_CALLBACK,
            Kind::Deny => ACE_TYPE_ACCESS_DENIED_CALLBACK,
        }
    }
}

struct ParsedArgs {
    specs: Vec<(Sid, u32)>,
    flags_override: Option<u8>,
    replace: bool,
    condition: Option<Condition>,
}

pub fn run(matches: &ArgMatches, kind: Kind) -> Result<()> {
    let primary = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);
    let recursive = walk::parse_recursive(matches);

    let raw_specs: Vec<String> = matches
        .get_many::<String>("specs")
        .map(|it| it.cloned().collect())
        .unwrap_or_default();
    if raw_specs.is_empty() {
        return Err(Error::Usage("no PRINCIPAL:PERMS specs given".into()));
    }
    let mut parsed_specs: Vec<(Sid, u32)> = Vec::with_capacity(raw_specs.len());
    for spec in &raw_specs {
        parsed_specs.push(parse_principal_perms(spec)?);
    }
    let condition = match matches.get_one::<String>("if") {
        Some(s) => Some(
            sddl::parse_condition(s).map_err(|e| Error::Invalid(format!("--if: {e}")))?,
        ),
        None => None,
    };
    let args = ParsedArgs {
        specs: parsed_specs,
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
        match apply_one(target, kind, &args) {
            Ok(n) => applied += n,
            Err(e) => errors.push((target.path.clone(), e)),
        }
    }

    emit_report(mode, kind, &targets, applied, &errors, args.replace)?;
    if !errors.is_empty() {
        // Recursive walk: surface a summary error for the worst case.
        return Err(Error::Usage(format!(
            "{} of {} target(s) failed",
            errors.len(),
            targets.len()
        )));
    }
    Ok(())
}

fn apply_one(target: &PathTarget, kind: Kind, args: &ParsedArgs) -> Result<usize> {
    let target_kind = TargetKind::from_path(&target.path)
        .map_err(|e| Error::NotFound(format!("{}: {e}", target.path)))?;
    let ace_flags = args
        .flags_override
        .unwrap_or_else(|| flags::default_for(target_kind));

    let new_dacl = if args.replace {
        let target_principals: Vec<Sid> =
            args.specs.iter().map(|(s, _)| s.clone()).collect();
        let dacl_ace_type = kind.ace_type();
        let cb_ace_type = kind.callback_ace_type();
        let (mut b, _dropped) = filter_acl(target, AclKind::Dacl, |ace| {
            if ace.ace_type != dacl_ace_type && ace.ace_type != cb_ace_type {
                return true;
            }
            if let Some((_, sid)) = ace.as_mask_sid() {
                let owned = sid.to_owned();
                if target_principals.contains(&owned) {
                    return false;
                }
            }
            true
        })?;
        for (sid, mask) in &args.specs {
            b = b.ace(build_ace(kind, sid.clone(), *mask, ace_flags, args.condition.as_ref())?);
        }
        b
    } else {
        let mut b = read_acl_builder(target, AclKind::Dacl)?;
        for (sid, mask) in &args.specs {
            b = b.ace(build_ace(kind, sid.clone(), *mask, ace_flags, args.condition.as_ref())?);
        }
        b
    };

    write_acl(target, AclKind::Dacl, new_dacl)?;
    Ok(args.specs.len())
}

fn build_ace(
    kind: Kind,
    sid: Sid,
    mask: u32,
    flags: u8,
    condition: Option<&Condition>,
) -> Result<AceBuilder> {
    let b = match (kind, condition) {
        (Kind::Allow, None) => AceBuilder::allow(sid, mask),
        (Kind::Deny, None) => AceBuilder::deny(sid, mask),
        (Kind::Allow, Some(c)) => AceBuilder::allow_callback(sid, mask, c)
            .map_err(|e| Error::Invalid(format!("conditional allow ACE: {e}")))?,
        (Kind::Deny, Some(c)) => AceBuilder::deny_callback(sid, mask, c)
            .map_err(|e| Error::Invalid(format!("conditional deny ACE: {e}")))?,
    };
    Ok(b.flags(flags))
}

fn parse_principal_perms(spec: &str) -> Result<(Sid, u32)> {
    let (princ, perms) = spec.rsplit_once(':').ok_or_else(|| {
        Error::Usage(format!(
            "spec `{spec}` needs `:` between principal and perms"
        ))
    })?;
    let sid = principal::parse(princ)?;
    let mask = perms::parse(perms)?;
    Ok((sid, mask))
}

fn emit_report(
    mode: OutputMode,
    kind: Kind,
    targets: &[PathTarget],
    applied: usize,
    errors: &[(String, Error)],
    replace: bool,
) -> Result<()> {
    match mode {
        OutputMode::Human => {
            if targets.len() == 1 && errors.is_empty() {
                println!(
                    "{}: {} ACE(s) {}ed",
                    targets[0].path,
                    applied,
                    kind.verb()
                );
            } else {
                println!(
                    "{}: {} ACE(s) {}ed across {} target(s)",
                    targets.first().map(|t| t.path.as_str()).unwrap_or("?"),
                    applied,
                    kind.verb(),
                    targets.len()
                );
                for (p, e) in errors {
                    eprintln!("  {p}: {e}");
                }
            }
        }
        OutputMode::Json => {
            let err_entries: Vec<_> = errors
                .iter()
                .map(|(p, e)| json!({ "path": p, "error": e.to_string() }))
                .collect();
            let v = json!({
                "verb": kind.verb(),
                "targets": targets.len(),
                "applied_aces": applied,
                "replace": replace,
                "errors": err_entries,
            });
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
    }
    Ok(())
}
