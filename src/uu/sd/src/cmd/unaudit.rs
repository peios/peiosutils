// `sd unaudit <path> <principal> [...]` — drop SACL audit ACEs for the
// listed principals. Supports --recursive.

use crate::cmd::dacl::{AclKind, filter_acl, write_acl};
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

    let principals: Vec<Sid> = matches
        .get_many::<String>("principals")
        .ok_or_else(|| Error::Usage("no principals given".into()))?
        .map(|s| principal::parse(s))
        .collect::<Result<Vec<_>>>()?;

    let targets = walk::walk_paths(&primary, recursive)?;
    let mut total = 0usize;
    let mut errors: Vec<(String, Error)> = Vec::new();
    for target in &targets {
        match apply_one(target, &principals) {
            Ok(n) => total += n,
            Err(e) => errors.push((target.path.clone(), e)),
        }
    }

    match mode {
        OutputMode::Human => {
            println!(
                "{}: dropped {} audit ACE(s) across {} target(s)",
                primary.path,
                total,
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
                    "dropped": total,
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

fn apply_one(target: &PathTarget, principals: &[Sid]) -> Result<usize> {
    let (new_sacl, dropped) = filter_acl(target, AclKind::Sacl, |ace| {
        if let Some((_, sid)) = ace.as_mask_sid() {
            let owned = sid.to_owned();
            !principals.contains(&owned)
        } else {
            true
        }
    })?;
    write_acl(target, AclKind::Sacl, new_sacl)?;
    Ok(dropped)
}
