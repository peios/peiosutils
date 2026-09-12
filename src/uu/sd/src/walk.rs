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
///
/// Every `--recursive` run arrives here with `no_follow_symlinks` already set,
/// because `parse_path_target` makes `-r` imply it — see the reasoning there.
/// `sd propagate`, which is recursive without saying so, is the one caller that
/// still passes whatever the operator asked for.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    /// The shape that produced PEI-584: a directory of symlinks pointing at one
    /// binary. The walk must yield each link, so the link inode is what gets
    /// stamped, and must not reach the target.
    #[test]
    fn a_no_follow_walk_yields_links_and_never_their_targets() {
        let root = tempdir().unwrap();
        let elsewhere = tempdir().unwrap();
        fs::write(elsewhere.path().join("multicall"), b"x").unwrap();
        fs::create_dir(elsewhere.path().join("outside")).unwrap();
        fs::write(elsewhere.path().join("outside/unrelated"), b"x").unwrap();

        fs::create_dir(root.path().join("sub")).unwrap();
        fs::write(root.path().join("sub/real"), b"x").unwrap();
        symlink(elsewhere.path().join("multicall"), root.path().join("whoami")).unwrap();
        symlink(elsewhere.path(), root.path().join("away")).unwrap();

        let target = PathTarget {
            path: root.path().to_string_lossy().into_owned(),
            no_follow_symlinks: true,
        };
        let targets = walk_paths(&target, true).unwrap();
        let paths: Vec<&str> = targets.iter().map(|t| t.path.as_str()).collect();

        assert!(paths.iter().any(|p| p.ends_with("/whoami")), "{paths:?}");
        assert!(paths.iter().any(|p| p.ends_with("/away")), "{paths:?}");
        assert!(paths.iter().any(|p| p.ends_with("/sub/real")), "{paths:?}");
        // Descent stopped at the directory symlink rather than walking out of
        // the tree it was asked to stamp.
        assert!(!paths.iter().any(|p| p.contains("unrelated")), "{paths:?}");

        // And each one stamps the inode at the path, not whatever it points to.
        assert!(
            targets
                .iter()
                .all(|t| t.at_flags() == libc::AT_SYMLINK_NOFOLLOW),
            "a descendant lost the no-follow flag"
        );
    }

    /// The behaviour the flag still buys when it is not set: descent through a
    /// directory symlink. `sd propagate` is the caller that can reach this.
    #[test]
    fn a_following_walk_descends_through_a_directory_symlink() {
        let root = tempdir().unwrap();
        let elsewhere = tempdir().unwrap();
        fs::write(elsewhere.path().join("reached"), b"x").unwrap();
        symlink(elsewhere.path(), root.path().join("away")).unwrap();

        let target = PathTarget {
            path: root.path().to_string_lossy().into_owned(),
            no_follow_symlinks: false,
        };
        let targets = walk_paths(&target, true).unwrap();
        let paths: Vec<&str> = targets.iter().map(|t| t.path.as_str()).collect();
        assert!(paths.iter().any(|p| p.ends_with("/away/reached")), "{paths:?}");
    }

    #[test]
    fn a_non_recursive_walk_is_just_the_root() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("child"), b"x").unwrap();
        let target = PathTarget {
            path: root.path().to_string_lossy().into_owned(),
            no_follow_symlinks: false,
        };
        assert_eq!(walk_paths(&target, false).unwrap().len(), 1);
    }
}
