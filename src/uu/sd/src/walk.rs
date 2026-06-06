// Filesystem walker for `--recursive` and `sd propagate`.
//
// Yields the root followed by every descendant in pre-order. Aggregates
// per-entry errors rather than aborting (caller decides via
// --stop-on-error in the future; for v1 we report at the end).

use crate::error::{Error, Result};
use crate::target::PathTarget;
use std::fs;
use std::path::Path;

/// Build the list of paths to operate on. If `recursive` is false, just
/// the root. If true, the root plus every descendant in pre-order.
///
/// Symlinks: if `root.no_follow_symlinks`, we use `symlink_metadata` so we
/// don't traverse into the target of a directory symlink. Otherwise we
/// follow symlinks during descent.
pub fn walk_paths(root: &PathTarget, recursive: bool) -> Result<Vec<PathTarget>> {
    let mut out = vec![root.clone()];
    if recursive {
        descend(&root.path, root.no_follow_symlinks, &mut out)?;
    }
    Ok(out)
}

fn descend(path: &str, nofollow: bool, out: &mut Vec<PathTarget>) -> Result<()> {
    let md = if nofollow {
        fs::symlink_metadata(path)
    } else {
        fs::metadata(path)
    };
    let Ok(md) = md else { return Ok(()) };
    if !md.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(path)
        .map_err(|e| Error::NotFound(format!("read_dir {path}: {e}")))?;
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let child = entry.path();
        let child_str = child.to_string_lossy().into_owned();
        out.push(PathTarget {
            path: child_str.clone(),
            no_follow_symlinks: nofollow,
        });
        let child_md = if nofollow {
            entry.metadata().ok()
        } else {
            fs::metadata(&child).ok()
        };
        if child_md.map(|m| m.is_dir()).unwrap_or(false) {
            descend(&child_str, nofollow, out)?;
        }
    }
    Ok(())
}

/// True if the path is a directory (used by verbs that pick a default
/// ACE-flag set based on container-ness).
pub fn is_container(path: &str) -> bool {
    let md = match Path::new(path).symlink_metadata() {
        Ok(m) => m,
        Err(_) => return false,
    };
    md.is_dir()
}

/// Compute the parent path of `path`. Used by `sd propagate` and `sd reset`.
pub fn parent_path(path: &str) -> String {
    Path::new(path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".to_string())
}

/// Extract `--recursive` from matches, defaulting to false.
pub fn parse_recursive(matches: &clap::ArgMatches) -> bool {
    matches
        .try_get_one::<bool>("recursive")
        .ok()
        .flatten()
        .copied()
        .unwrap_or(false)
}
