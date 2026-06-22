// Path target resolution.
//
// v1 surface: filesystem paths. The new `peios::file::{get_sd,set_sd}` take
// `(dirfd: Option<BorrowedFd>, path: &Path, SecInfo, at_flags: i32)`; a
// `PathTarget` lowers to those three: dirfd is always `None` (process cwd,
// the old `FDCWD`), the path, and `AT_SYMLINK_NOFOLLOW` when requested.

use std::os::fd::BorrowedFd;
use std::path::Path;

/// What `sd` is operating on. v1: paths only.
#[derive(Debug, Clone)]
pub struct PathTarget {
    pub path: String,
    pub no_follow_symlinks: bool,
}

impl PathTarget {
    /// The directory fd to resolve relative to — always `None` (the process
    /// cwd, i.e. the old `raw::FDCWD`).
    pub fn dirfd(&self) -> Option<BorrowedFd<'static>> {
        None
    }

    /// The path as an `&Path`, as `get_sd` / `set_sd` expect.
    pub fn as_path(&self) -> &Path {
        Path::new(&self.path)
    }

    /// The `*at` flags: `AT_SYMLINK_NOFOLLOW` when operating on the link itself.
    pub fn at_flags(&self) -> i32 {
        if self.no_follow_symlinks {
            libc::AT_SYMLINK_NOFOLLOW
        } else {
            0
        }
    }
}
