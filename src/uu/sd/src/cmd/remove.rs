// `sd remove <path> <principal> [...]` — drop every DACL ACE for the
// listed principals (both allow and deny). Refuses to leave a
// present-but-empty DACL unless --allow-empty. Supports --recursive.

use crate::cmd::dacl::{AclKind, filter_acl, is_explicit, write_acl};
use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::principal;
use crate::target::PathTarget;
use crate::walk;
use clap::ArgMatches;
use libp_sd::Sid;
use serde_json::json;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let primary = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);
    let recursive = walk::parse_recursive(matches);
    let allow_empty = matches.get_flag("allow-empty");

    let principals: Vec<Sid> = matches
        .get_many::<String>("principals")
        .ok_or_else(|| Error::Usage("no principals given".into()))?
        .map(|s| principal::parse(s))
        .collect::<Result<Vec<_>>>()?;

    let targets = walk::walk_paths(&primary, recursive)?;
    let mut total_dropped = 0usize;
    let mut errors: Vec<(String, Error)> = Vec::new();
    for target in &targets {
        match apply_one(target, &principals, allow_empty) {
            Ok(n) => total_dropped += n,
            Err(e) => errors.push((target.path.clone(), e)),
        }
    }

    match mode {
        OutputMode::Human => {
            println!(
                "{}: dropped {} ACE(s) across {} target(s)",
                primary.path,
                total_dropped,
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
                    "total_dropped": total_dropped,
                    "principals": principals.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
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

fn apply_one(target: &PathTarget, principals: &[Sid], allow_empty: bool) -> Result<usize> {
    let (new_dacl, dropped) = filter_acl(target, AclKind::Dacl, |ace| {
        if let Some((_, sid)) = ace.as_mask_sid() {
            let owned = sid.to_owned();
            !principals.contains(&owned)
        } else {
            true
        }
    })?;

    if new_dacl.is_empty() && !allow_empty {
        let (_, also_dropped_inherited) = filter_acl(target, AclKind::Dacl, |ace| {
            !is_explicit(ace) || ace.as_mask_sid().is_none()
        })?;
        if dropped > 0 || also_dropped_inherited > 0 {
            return Err(Error::Usage(format!(
                "{}: refusing to leave a present-but-empty DACL. \
                Pass --allow-empty to override, or `sd inherit on` / \
                `sd reset` to re-derive from parent",
                target.path
            )));
        }
    }

    write_acl(target, AclKind::Dacl, new_dacl)?;
    Ok(dropped)
}
