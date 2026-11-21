// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Deleting through a KACS handle, for what a plain `unlink` cannot do.
//!
//! - **A read-only regular file.** KACS refuses to unlink one, for every
//!   process. rm deletes it, once the user has agreed or given `-f`, in one
//!   step: a handle opened with `DELETE` and `FILE_WRITE_ATTRIBUTES`, then a
//!   delete disposition with `IGNORE_READONLY`. The flag is never cleared
//!   first, so a file rm then fails to delete is not left writable.
//! - **`--when-closed`.** The delete disposition without `POSIX_SEMANTICS`
//!   marks the file or directory for deletion: its name stays until the last
//!   open of it closes, and new opens fail meanwhile. A directory is removed
//!   then only if it is empty.
//!
//! See "Sharing and Deletion" and "Special Attributes" in the kernel's KACS
//! file-security reference.

use std::io;
use std::os::fd::BorrowedFd;
use std::path::Path;

use peios::file::{
    CreateOptions, DeleteDisposition, Disposition, File, FileAccess, OpenFailure, OpenFlags,
    OpenOptions, ShareAccess,
};
use uucore::special_attributes;
use uucore::translate;

/// Whether `name`, in the directory `dir` or the current directory with
/// `None`, is read-only. A symbolic link at `name` is not followed. A file
/// whose attributes cannot be read is taken not to be: a plain unlink then
/// reports whatever is wrong.
pub(crate) fn is_read_only(dir: Option<BorrowedFd<'_>>, name: &Path) -> bool {
    special_attributes::read_at(dir, name, false).is_ok_and(|attrs| attrs.read_only)
}

/// What became of a name deleted through a handle.
pub(crate) enum Deleted {
    /// It is gone.
    Now,
    /// Another program still has it open; it goes when the last one closes it.
    WhenClosed,
}

/// Delete the regular file or directory `name`, in `dir` or the current
/// directory, through a handle: at once, or, with `when_closed`, when the
/// last open of it closes. `read_only` sets the file's read-only flag aside.
pub(crate) fn delete(
    dir: Option<BorrowedFd<'_>>,
    name: &Path,
    is_dir: bool,
    read_only: bool,
    when_closed: bool,
) -> io::Result<Deleted> {
    let mut access = FileAccess::DELETE | FileAccess::SYNCHRONIZE;
    let mut disposition = DeleteDisposition::DELETE;
    if read_only {
        access |= FileAccess::WRITE_ATTRIBUTES;
        disposition |= DeleteDisposition::IGNORE_READONLY;
    }
    if !when_closed {
        disposition |= DeleteDisposition::POSIX_SEMANTICS;
    }

    let file = open(dir, name, access, is_dir).map_err(in_use)?;
    file.set_delete_disposition(disposition)
        .map_err(io::Error::from)?;
    drop(file);
    if !when_closed {
        return Ok(Deleted::Now);
    }

    // rm cannot see other programs' opens, so it asks the name: while the
    // deletion is pending, an open of it fails with DELETE_PENDING.
    match open(
        dir,
        name,
        FileAccess::READ_ATTRIBUTES | FileAccess::SYNCHRONIZE,
        is_dir,
    ) {
        Err(e) if e.open_failure() == Some(OpenFailure::DeletePending) => Ok(Deleted::WhenClosed),
        Err(e) if e.raw_os_error() == Some(libc::ENOENT) => Ok(Deleted::Now),
        Err(e) => Err(e.into()),
        // A directory still in place, and not pending, was not empty when rm
        // closed it: entries marked in it are held open elsewhere.
        Ok(_) if is_dir => Err(io::Error::other(translate!(
            "rm-error-directory-not-emptied"
        ))),
        // Something new at the name; the file rm marked is gone.
        Ok(_) => Ok(Deleted::Now),
    }
}

fn open(
    dir: Option<BorrowedFd<'_>>,
    name: &Path,
    access: FileAccess,
    is_dir: bool,
) -> peios::Result<File> {
    let mut options = OpenOptions::new();
    options
        .desired_access(access)
        .disposition(Disposition::Open)
        // Other programs may go on using the file meanwhile: rm asks only
        // that it be deleted.
        .share_access(ShareAccess::READ | ShareAccess::WRITE | ShareAccess::DELETE)
        .flags(OpenFlags::SYMLINK_NOFOLLOW);
    if is_dir {
        options.options(CreateOptions::DIRECTORY);
    }
    options.open(dir, name)
}

/// An open refused because another open of the file does not share delete,
/// said as such.
fn in_use(e: peios::Error) -> io::Error {
    if e.open_failure() == Some(OpenFailure::SharingViolation) {
        io::Error::other(translate!("rm-error-in-use"))
    } else {
        e.into()
    }
}

/// A plain unlink of a regular file refused with `EBUSY`, which KACS gives
/// when another open of it does not share delete, said as such. Anything
/// else, and any directory, whose `EBUSY` means a mount point, is returned
/// as it is.
pub(crate) fn explain_busy(e: io::Error, is_dir: bool) -> io::Error {
    if !is_dir && e.raw_os_error() == Some(libc::EBUSY) {
        io::Error::other(translate!("rm-error-in-use"))
    } else {
        e
    }
}
