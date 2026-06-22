// `sd allow` and `sd deny` share the same surface — only the ACE kind
// differs. Implemented together. Supports --recursive and --if.

use crate::cmd::dacl::{AclKind, ace_mask_sid, filter_acl, read_acl_builder, write_acl};
use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::flags::{self, TargetKind};
use crate::perms;
use crate::principal;
use crate::target::PathTarget;
use crate::walk;
use clap::ArgMatches;
use peios::security::{AceType, Sid, sddl};
use serde_json::json;

// Callback ACE-type wire discriminants (no typed `AceType` variant for these).
const ACE_TYPE_ACCESS_ALLOWED_CALLBACK: u8 = peios_sys::KACS_ACE_TYPE_ACCESS_ALLOWED_CALLBACK as u8;
const ACE_TYPE_ACCESS_DENIED_CALLBACK: u8 = peios_sys::KACS_ACE_TYPE_ACCESS_DENIED_CALLBACK as u8;

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Allow,
    Deny,
}

impl Kind {
    /// The plain (non-conditional) ACE type.
    fn ace_type(self) -> AceType {
        match self {
            Kind::Allow => AceType::AccessAllowed,
            Kind::Deny => AceType::AccessDenied,
        }
    }
    fn verb(self) -> &'static str {
        match self {
            Kind::Allow => "allow",
            Kind::Deny => "deny",
        }
    }
    /// The conditional (callback) ACE-type discriminant.
    fn callback_ace_type(self) -> u8 {
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
    /// The `artx` condition bytecode from `--if`, if any.
    condition: Option<Vec<u8>>,
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

    let mut new_dacl = if args.replace {
        let target_principals: Vec<Sid> = args.specs.iter().map(|(s, _)| s.clone()).collect();
        let dacl_ace_type = kind.ace_type();
        let cb_ace_type = AceType::Other(kind.callback_ace_type());
        let (b, _dropped) = filter_acl(target, AclKind::Dacl, |ace| {
            let t = ace.ace_type();
            if t != dacl_ace_type && t != cb_ace_type {
                return true;
            }
            if let Some((_, sid)) = ace_mask_sid(ace) {
                if target_principals.contains(&sid) {
                    return false;
                }
            }
            true
        })?;
        b
    } else {
        read_acl_builder(target, AclKind::Dacl)?
    };

    for (sid, mask) in &args.specs {
        add_ace(&mut new_dacl, kind, sid, *mask, ace_flags, args.condition.as_deref());
    }

    write_acl(target, AclKind::Dacl, new_dacl)?;
    Ok(args.specs.len())
}

fn add_ace(
    acl: &mut crate::cmd::dacl::AclEdit,
    kind: Kind,
    sid: &Sid,
    mask: u32,
    flags: u8,
    condition: Option<&[u8]>,
) {
    match (kind, condition) {
        (Kind::Allow, None) => acl.allow(sid, mask, flags),
        (Kind::Deny, None) => acl.deny(sid, mask, flags),
        (_, Some(artx)) => acl.callback(kind.callback_ace_type(), sid, mask, flags, artx),
    }
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
