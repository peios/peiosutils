// `sd propagate <path>` — walk descendants and reinherit each from its
// parent. Tool-side because the kernel has no reinheritance primitive.
//
// The root path itself isn't touched (the root's DACL is the *source*
// for inheritance into its children). For each descendant we:
//   1. get_sd(parent) — DACL only
//   2. get_sd(child)  — DACL only
//   3. libp_sd::reinherit(parent, child, child_is_container)
//   4. set_sd(child, dacl)

use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::target::PathTarget;
use crate::walk;
use clap::ArgMatches;
use libp_sd::{SecurityInfo, get_sd, reinherit, set_sd};
use serde_json::json;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let root = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);

    // The walker yields [root, ...descendants]. We skip the root because
    // propagation pushes *down*; the root's own DACL is the source.
    let all = walk::walk_paths(&root, true)?;
    if all.len() == 1 {
        return Err(Error::Usage(format!(
            "{}: not a directory, nothing to propagate",
            root.path
        )));
    }
    let descendants = &all[1..];

    let mut pushed = 0usize;
    let mut errors: Vec<(String, Error)> = Vec::new();
    for child in descendants {
        match push_one(child, root.no_follow_symlinks) {
            Ok(()) => pushed += 1,
            Err(e) => errors.push((child.path.clone(), e)),
        }
    }

    match mode {
        OutputMode::Human => {
            println!(
                "{}: propagated inheritance to {} descendant(s){}",
                root.path,
                pushed,
                if errors.is_empty() {
                    String::new()
                } else {
                    format!(" ({} error(s))", errors.len())
                }
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
                    "root": root.path,
                    "descendants": descendants.len(),
                    "pushed": pushed,
                    "errors": err_entries,
                }))
                .unwrap()
            );
        }
    }

    if !errors.is_empty() {
        return Err(Error::Usage(format!(
            "{} of {} descendants failed",
            errors.len(),
            descendants.len()
        )));
    }
    Ok(())
}

fn push_one(child: &PathTarget, nofollow: bool) -> Result<()> {
    let parent_path = walk::parent_path(&child.path);
    let parent_target = PathTarget {
        path: parent_path,
        no_follow_symlinks: nofollow,
    };
    let parent_bytes =
        get_sd(&parent_target.as_sd_target(), SecurityInfo::dacl()).map_err(Error::from)?;
    let child_bytes =
        get_sd(&child.as_sd_target(), SecurityInfo::dacl()).map_err(Error::from)?;
    let child_is_container = walk::is_container(&child.path);
    let new_sd = if child_bytes.is_empty() {
        // Child has no SD yet; nothing to reinherit on top of.
        return Ok(());
    } else if parent_bytes.is_empty() {
        // Parent has no SD — strip child's stale inherited ACEs only.
        let empty = empty_self_relative_sd();
        reinherit(&empty, &child_bytes, child_is_container).map_err(Error::from)?
    } else {
        reinherit(&parent_bytes, &child_bytes, child_is_container).map_err(Error::from)?
    };
    set_sd(&child.as_sd_target(), SecurityInfo::dacl(), &new_sd).map_err(Error::from)?;
    Ok(())
}

fn empty_self_relative_sd() -> Vec<u8> {
    use libp_sd::consts::{SD_HEADER_BYTES, SE_SELF_RELATIVE};
    let mut out = vec![0u8; SD_HEADER_BYTES];
    out[0] = 1;
    out[2..4].copy_from_slice(&SE_SELF_RELATIVE.to_le_bytes());
    out
}
