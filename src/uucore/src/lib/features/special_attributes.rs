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
use std::os::fd::BorrowedFd;
use std::path::Path;
use std::sync::OnceLock;
use std::time::SystemTime;

use peios::file;

pub use peios::file::SpecialAttributes;

fn at_flags(follow: bool) -> i32 {
    if follow { 0 } else { libc::AT_SYMLINK_NOFOLLOW }
}

/// Whether `e` means the file has no special attributes to read or set:
/// its file system is not managed by KACS, or the kernel has no KACS. On a
/// kernel without KACS nothing handles `security.peios.sattr`: reading it
/// finds nothing, and writing it fails as for any `security.*` attribute no
/// security module owns, with `EPERM`. On Peios, `EPERM` is a real failure.
pub fn is_unsupported(e: &io::Error) -> bool {
    match e.raw_os_error() {
        Some(libc::EOPNOTSUPP | libc::ENOSYS | libc::ENOTTY) => true,
        Some(libc::EPERM) => !kacs_present(),
        _ => false,
    }
}

/// Whether the running kernel has KACS: a KACS call, for the root
/// directory's owner, is not refused as an unknown system call. Asked once.
fn kacs_present() -> bool {
    static PRESENT: OnceLock<bool> = OnceLock::new();
    *PRESENT.get_or_init(|| {
        !matches!(
            file::get_sd(None, Path::new("/"), file::SecInfo::OWNER, 0),
            Err(e) if e.raw_os_error() == Some(libc::ENOSYS)
        )
    })
}

/// The special attributes of `path`. A file without the record has none.
pub fn read(path: &Path, follow: bool) -> io::Result<SpecialAttributes> {
    read_at(None, path, follow)
}

/// The special attributes of `path` in the directory `dir`, or in the current
/// directory with `None`. A file without the record has none.
pub fn read_at(
    dir: Option<BorrowedFd<'_>>,
    path: &Path,
    follow: bool,
) -> io::Result<SpecialAttributes> {
    match file::special_attributes(dir, path, at_flags(follow)) {
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

// The same, for the file a descriptor refers to, so a caller can work on an
// object it opened once without naming it again. Not an `O_PATH` descriptor:
// the kernel takes an empty path here as `f*xattr` does, which refuses one
// with `EBADF`.

/// The special attributes of the file `fd` refers to.
pub fn read_fd(fd: BorrowedFd<'_>) -> io::Result<SpecialAttributes> {
    match file::special_attributes(Some(fd), Path::new(""), libc::AT_EMPTY_PATH) {
        Ok(attrs) => Ok(attrs),
        Err(e) if e.raw_os_error() == Some(libc::ENODATA) => Ok(SpecialAttributes::default()),
        Err(e) => Err(e.into()),
    }
}

/// The creation date of the file `fd` refers to. See [`creation_time`].
pub fn creation_time_fd(fd: BorrowedFd<'_>) -> io::Result<Option<SystemTime>> {
    match file::creation_time(Some(fd), Path::new(""), libc::AT_EMPTY_PATH) {
        Ok(time) => Ok(Some(time)),
        Err(e) if e.raw_os_error() == Some(libc::ENODATA) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Make the file `fd` refers to read-only, or not.
pub fn set_read_only_fd(fd: BorrowedFd<'_>, read_only: bool) -> io::Result<()> {
    file::set_read_only(Some(fd), Path::new(""), read_only, libc::AT_EMPTY_PATH)
        .map_err(io::Error::from)
}

/// Set or unset the Created At of the file `fd` refers to.
pub fn set_created_at_fd(fd: BorrowedFd<'_>, created_at: Option<SystemTime>) -> io::Result<()> {
    file::set_created_at(Some(fd), Path::new(""), created_at, libc::AT_EMPTY_PATH)
        .map_err(io::Error::from)
}

/// Delete the read-only regular file `path`, without following a symbolic
/// link there. KACS refuses a plain unlink of a read-only file; this deletes
/// it through a handle, with a delete disposition that sets the flag aside.
/// That needs `DELETE` and `FILE_WRITE_ATTRIBUTES` on the file, the right
/// that would clear the flag. For a caller that has decided the file goes,
/// read-only or not, as a move does once the file is copied elsewhere.
pub fn delete_read_only(path: &Path) -> io::Result<()> {
    let mut options = file::OpenOptions::new();
    options
        .desired_access(
            file::FileAccess::DELETE
                | file::FileAccess::WRITE_ATTRIBUTES
                | file::FileAccess::SYNCHRONIZE,
        )
        .disposition(file::Disposition::Open)
        .share_access(
            file::ShareAccess::READ | file::ShareAccess::WRITE | file::ShareAccess::DELETE,
        )
        .flags(file::OpenFlags::SYMLINK_NOFOLLOW);
    let handle = options.open(None, path)?;
    handle.set_delete_disposition(
        file::DeleteDisposition::DELETE
            | file::DeleteDisposition::POSIX_SEMANTICS
            | file::DeleteDisposition::IGNORE_READONLY,
    )?;
    Ok(())
}
