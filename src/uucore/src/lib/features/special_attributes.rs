// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! A file's KACS special attributes: read-only and Created At.
//!
//! KACS keeps both in one record, the `security.peios.sattr` extended
//! attribute. Read-only is enforced for every process on a regular file: it
//! cannot be opened for writing or deleted until the flag is cleared.
//! Created At is a settable creation date, distinct from the filesystem's
//! birth time, which nobody can set; when it is unset, a file's creation
//! date reads as its birth time. See "Special Attributes" in the kernel's
//! KACS file-security reference.
//!
//! The functions here are the one place the tools read and write the record,
//! so they all treat its edge cases alike:
//!
//! - A file that has never had the record reads as having no attributes:
//!   not read-only, Created At unset. KACS reports that as `ENODATA`, which
//!   is not an error to a caller.
//! - A file system KACS does not manage, or a kernel without KACS, has no
//!   special attributes at all. That stays an error, and
//!   [`is_unsupported`] names it, so a tool can decide whether to fail or
//!   to carry on without them.
//!
//! Each function takes `follow`: whether a symbolic link at `path` is
//! followed, as `stat -L` or `chmod` would, or the link itself is meant.

use std::io;
use std::path::Path;
use std::time::SystemTime;

use peios::file;

pub use peios::file::SpecialAttributes;

fn at_flags(follow: bool) -> i32 {
    if follow { 0 } else { libc::AT_SYMLINK_NOFOLLOW }
}

/// Whether `e` means the file has no special attributes to read or set:
/// its file system is not managed by KACS, or the kernel has no KACS.
pub fn is_unsupported(e: &io::Error) -> bool {
    matches!(
        e.raw_os_error(),
        Some(libc::EOPNOTSUPP | libc::ENOSYS | libc::ENOTTY)
    )
}

/// The special attributes of `path`. A file without the record has none.
pub fn read(path: &Path, follow: bool) -> io::Result<SpecialAttributes> {
    match file::special_attributes(None, path, at_flags(follow)) {
        Ok(attrs) => Ok(attrs),
        Err(e) if e.raw_os_error() == Some(libc::ENODATA) => Ok(SpecialAttributes::default()),
        Err(e) => Err(e.into()),
    }
}

/// The creation date of `path`: its Created At if set, else its birth time.
/// `None` if it has neither.
pub fn creation_time(path: &Path, follow: bool) -> io::Result<Option<SystemTime>> {
    match file::creation_time(None, path, at_flags(follow)) {
        Ok(time) => Ok(Some(time)),
        Err(e) if e.raw_os_error() == Some(libc::ENODATA) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Make `path` read-only, or not, leaving Created At alone.
pub fn set_read_only(path: &Path, read_only: bool, follow: bool) -> io::Result<()> {
    file::set_read_only(None, path, read_only, at_flags(follow)).map_err(io::Error::from)
}

/// Set the Created At of `path`, or unset it with `None`, leaving read-only
/// alone.
pub fn set_created_at(path: &Path, created_at: Option<SystemTime>, follow: bool) -> io::Result<()> {
    file::set_created_at(None, path, created_at, at_flags(follow)).map_err(io::Error::from)
}
