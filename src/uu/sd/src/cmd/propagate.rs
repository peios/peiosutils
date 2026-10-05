// `sd propagate <path>` — walk descendants and re-propagate each from its
// parent (PCDS §5.6, Re-propagation). Tool-side because the kernel has no
// re-propagation primitive.
//
// The root path itself isn't touched (the root's lists are the *source*
// for inheritance into its children). Descendants come parents first, so
// each is re-propagated from its parent as just rewritten. For each:
//   1. get_sd(parent), get_sd(child) — the DACL, and the SACL with --sacl
//   2. reinherit_with: what KACS gives a child it creates, through the file
//      generic mapping, leaving a list the child protects as it is
//   3. set_sd(child) — the lists it changed, unless it protects them all

use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::target::PathTarget;
use crate::walk;
use clap::ArgMatches;
use peios::file::{File, SecInfo, get_sd, set_sd};
use peios::security::{Control, SdView, reinherit_with};
use serde_json::json;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let root = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);
    let lists = if matches.get_flag("sacl") { SecInfo::DACL | SecInfo::SACL } else { SecInfo::DACL };

    // The walker yields [root, ...descendants]. We skip the root because
    // propagation pushes *down*; the root's own lists are the source.
    let all = walk::walk_paths(&root, true)?;
    if all.len() == 1 {
        return Err(Error::Usage(format!(
            "{}: not a directory, nothing to propagate",
            root.path
        )));
    }
    let descendants = &all[1..];

    let mut pushed = 0usize;
    let mut protected = 0usize;
    let mut errors: Vec<(String, Error)> = Vec::new();
    for child in descendants {
        match push_one(child, root.no_follow_symlinks, lists) {
            Ok(true) => pushed += 1,
            Ok(false) => protected += 1,
            Err(e) => errors.push((child.path.clone(), e)),
        }
    }

    match mode {
        OutputMode::Human => {
            println!(
                "{}: propagated inheritance to {} descendant(s){}{}",
                root.path,
                pushed,
                if protected == 0 { String::new() } else { format!(", {protected} protected and left as they are") },
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
                    "protected": protected,
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

/// Re-propagates one descendant. `false` where it protects every list
/// asked for, and so is left as it is.
fn push_one(child: &PathTarget, nofollow: bool, lists: SecInfo) -> Result<bool> {
    let parent_path = walk::parent_path(&child.path);
    let parent_target = PathTarget {
        path: parent_path,
        no_follow_symlinks: nofollow,
    };
    let parent_sd = get_sd(
        parent_target.dirfd(),
        parent_target.as_path(),
        lists,
        parent_target.at_flags(),
    )
    .map_err(Error::from)?;
    // The owner and group too, which CREATOR OWNER and CREATOR GROUP
    // resolve to.
    let child_sd = get_sd(child.dirfd(), child.as_path(), lists | SecInfo::OWNER | SecInfo::GROUP, child.at_flags())
        .map_err(Error::from)?;
    let child_bytes = child_sd.as_bytes();
    if child_bytes.is_empty() {
        // Child has no SD yet; nothing to reinherit on top of.
        return Ok(true);
    }
    let control = SdView::parse(child_bytes).map_err(Error::from)?.control();
    let mut write = lists;
    if control.contains(Control::DACL_PROTECTED) {
        write.remove(SecInfo::DACL);
    }
    if control.contains(Control::SACL_PROTECTED) {
        write.remove(SecInfo::SACL);
    }
    if write.is_empty() {
        return Ok(false);
    }
    let empty;
    let parent_bytes = if parent_sd.as_bytes().is_empty() {
        // Parent has no SD — strip child's stale inherited ACEs only.
        empty = crate::cmd::empty_self_relative_sd();
        &empty[..]
    } else {
        parent_sd.as_bytes()
    };
    let mapping = File::generic_mapping();
    let new_sd = reinherit_with(parent_bytes, child_bytes, walk::is_container(&child.path), Some(&mapping), write)
        .map_err(Error::from)?;
    set_sd(child.dirfd(), child.as_path(), write, &new_sd, child.at_flags())
        .map_err(Error::from)?;
    Ok(true)
}
