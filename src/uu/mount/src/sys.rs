// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Thin, unsafe-but-honest wrappers over the Linux fd-based mount API
//! (syscalls 428–433) plus `mount_setattr` (442).
//!
//! peios-sys binds the *libpeios* C ABI, not raw Linux syscalls, and the `libc`
//! crate (0.2) exposes the `SYS_*` numbers, the `mount_attr` struct and the
//! `OPEN_TREE_*` / `MOVE_MOUNT_*` / `MOUNT_ATTR_*` flags but neither the
//! `fsopen`/`fsconfig`/… glibc wrappers nor the `FSCONFIG_*` / `FSMOUNT_*` /
//! `FSPICK_*` command constants. So we declare those constants here and route
//! every call through `libc::syscall`. This is the only module that talks to
//! the kernel; everything above it is ordinary safe Rust.
//!
//! Each wrapper returns `io::Result`, mapping a negative return to the live
//! `errno` via [`io::Error::last_os_error`]. fd-returning calls hand back an
//! [`OwnedFd`] so the caller can't leak the detached mount.

use std::ffi::CStr;
use std::io;
use std::os::fd::{BorrowedFd, FromRawFd, OwnedFd, RawFd};

// ---------------------------------------------------------------------------
// Command / flag constants not provided by libc 0.2.
// Values are the stable UAPI ones from <linux/mount.h>.
// ---------------------------------------------------------------------------

/// `fsopen(2)` flags.
pub const FSOPEN_CLOEXEC: u32 = 0x0000_0001;

/// `fsconfig(2)` commands.
pub mod fsconfig_cmd {
    /// Set a flag (no value), e.g. `ro`.
    pub const SET_FLAG: u32 = 0;
    /// Set a string value, e.g. `source=/dev/sda1`.
    pub const SET_STRING: u32 = 1;
    /// Set a binary blob value.
    pub const SET_BINARY: u32 = 2;
    /// Set a path (resolved relative to `aux` dirfd).
    pub const SET_PATH: u32 = 3;
    /// Set a path, permitting an empty path with `AT_EMPTY_PATH` semantics.
    pub const SET_PATH_EMPTY: u32 = 4;
    /// Set a value to an open fd (`aux` is the fd).
    pub const SET_FD: u32 = 5;
    /// Realise the superblock from the accumulated config.
    pub const CMD_CREATE: u32 = 6;
    /// Reconfigure an existing superblock (remount path).
    pub const CMD_RECONFIGURE: u32 = 7;
    /// Like `CMD_CREATE` but fail (`EBUSY`) if it would reuse an existing
    /// superblock — the exclusive-create variant.
    pub const CMD_CREATE_EXCL: u32 = 8;
}

/// `fsmount(2)` flags.
pub const FSMOUNT_CLOEXEC: u32 = 0x0000_0001;

/// `fspick(2)` flags.
pub mod fspick {
    pub const CLOEXEC: u32 = 0x0000_0001;
    pub const SYMLINK_NOFOLLOW: u32 = 0x0000_0002;
    pub const NO_AUTOMOUNT: u32 = 0x0000_0004;
    pub const EMPTY_PATH: u32 = 0x0000_0008;
}

/// `AT_RECURSIVE` — apply to the whole subtree (open_tree / mount_setattr).
/// Not exposed by libc 0.2, so declared here (stable UAPI value).
pub const AT_RECURSIVE: u32 = 0x8000;

/// `open_tree(2)` flags (the rest come from `libc`: `OPEN_TREE_CLONE`,
/// `OPEN_TREE_CLOEXEC`).
pub use libc::{OPEN_TREE_CLOEXEC, OPEN_TREE_CLONE};

/// Mount propagation types, as written into `mount_attr.propagation`. These are
/// the classic `MS_*` propagation values (libc provides them as `c_ulong`).
pub mod propagation {
    pub const SHARED: u64 = libc::MS_SHARED;
    pub const SLAVE: u64 = libc::MS_SLAVE;
    pub const PRIVATE: u64 = libc::MS_PRIVATE;
    pub const UNBINDABLE: u64 = libc::MS_UNBINDABLE;
}

/// `umount2(2)` flags (from libc): `MNT_FORCE`, `MNT_DETACH`, `MNT_EXPIRE`,
/// `UMOUNT_NOFOLLOW`.
pub use libc::{MNT_DETACH, MNT_FORCE, UMOUNT_NOFOLLOW};

// ---------------------------------------------------------------------------
// fsopen → fsconfig → fsmount → move_mount  (the new-mount path)
// ---------------------------------------------------------------------------

/// `fsopen(2)` — create a detached filesystem-configuration context for `fstype`.
pub fn fsopen(fstype: &CStr, flags: u32) -> io::Result<OwnedFd> {
    // SAFETY: `fstype` is a valid NUL-terminated C string for the call's
    // duration; the kernel only reads it.
    let ret = unsafe { libc::syscall(libc::SYS_fsopen, fstype.as_ptr(), flags) };
    owned_fd(ret)
}

/// `fsconfig(2)` with `FSCONFIG_SET_STRING` — set `key=value` on the context.
pub fn fsconfig_set_string(fs_fd: BorrowedFd<'_>, key: &CStr, value: &CStr) -> io::Result<()> {
    // SAFETY: fd is borrowed (kept alive by the caller); key/value are valid
    // C strings; aux is unused (0) for SET_STRING.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_fsconfig,
            fd_arg(fs_fd),
            fsconfig_cmd::SET_STRING,
            key.as_ptr(),
            value.as_ptr(),
            0,
        )
    };
    check(ret)
}

/// `fsconfig(2)` with `FSCONFIG_SET_FLAG` — set a valueless flag like `ro`.
pub fn fsconfig_set_flag(fs_fd: BorrowedFd<'_>, key: &CStr) -> io::Result<()> {
    // SAFETY: as above; value/aux are NULL/0 for SET_FLAG.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_fsconfig,
            fd_arg(fs_fd),
            fsconfig_cmd::SET_FLAG,
            key.as_ptr(),
            std::ptr::null::<libc::c_void>(),
            0,
        )
    };
    check(ret)
}

/// `fsconfig(2)` with a bare command (`CMD_CREATE`, `CMD_CREATE_EXCL`,
/// `CMD_RECONFIGURE`) — key/value/aux all empty.
pub fn fsconfig_cmd(fs_fd: BorrowedFd<'_>, cmd: u32) -> io::Result<()> {
    // SAFETY: command-only fsconfig takes no key/value.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_fsconfig,
            fd_arg(fs_fd),
            cmd,
            std::ptr::null::<libc::c_char>(),
            std::ptr::null::<libc::c_void>(),
            0,
        )
    };
    check(ret)
}

/// `fsmount(2)` — turn a created superblock context into a detached mount fd.
///
/// `attr_flags` is a mask of `MOUNT_ATTR_*` (from `libc`).
pub fn fsmount(fs_fd: BorrowedFd<'_>, flags: u32, attr_flags: u32) -> io::Result<OwnedFd> {
    // SAFETY: fd borrowed for the call; scalars only.
    let ret = unsafe { libc::syscall(libc::SYS_fsmount, fd_arg(fs_fd), flags, attr_flags) };
    owned_fd(ret)
}

/// `AT_EMPTY_PATH` — operate on the fd itself with an empty path.
pub const AT_EMPTY_PATH: u32 = 0x1000;

/// Low-level `move_mount(2)`.
fn move_mount_raw(
    from_dfd: libc::c_long,
    from_path: &CStr,
    to_dfd: libc::c_long,
    to_path: &CStr,
    flags: u32,
) -> io::Result<()> {
    // SAFETY: both paths are valid C strings read only for the call.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_move_mount,
            from_dfd,
            from_path.as_ptr(),
            to_dfd,
            to_path.as_ptr(),
            flags,
        )
    };
    check(ret)
}

/// Graft a *detached* mount (an `fsmount`/`open_tree` fd) onto `target` —
/// empty `from_path` + `MOVE_MOUNT_F_EMPTY_PATH`.
pub fn move_mount_attach(from_fd: BorrowedFd<'_>, target: &CStr, extra_flags: u32) -> io::Result<()> {
    let empty = c"";
    move_mount_raw(
        fd_arg(from_fd),
        empty,
        libc::AT_FDCWD as libc::c_long,
        target,
        libc::MOVE_MOUNT_F_EMPTY_PATH | extra_flags,
    )
}

/// Relocate an existing mount named by `from` path to `target` (the `--move` /
/// `--beneath` path; `flags` carries `MOVE_MOUNT_BENEATH` etc.).
pub fn move_mount_path(from: &CStr, target: &CStr, flags: u32) -> io::Result<()> {
    move_mount_raw(
        libc::AT_FDCWD as libc::c_long,
        from,
        libc::AT_FDCWD as libc::c_long,
        target,
        flags,
    )
}

// ---------------------------------------------------------------------------
// open_tree / fspick / mount_setattr  (bind, remount, propagation — wired now,
// exercised by the layering phase)
// ---------------------------------------------------------------------------

/// `open_tree(2)` — clone (`OPEN_TREE_CLONE`) or reference a subtree as an fd.
pub fn open_tree(dfd: BorrowedFd<'_>, path: &CStr, flags: u32) -> io::Result<OwnedFd> {
    // SAFETY: fd borrowed for the call; path is a valid C string.
    let ret = unsafe { libc::syscall(libc::SYS_open_tree, fd_arg(dfd), path.as_ptr(), flags) };
    owned_fd(ret)
}

/// `fspick(2)` — open a configuration context against an *existing* mount, for
/// reconfiguration (the remount path).
pub fn fspick(dfd: BorrowedFd<'_>, path: &CStr, flags: u32) -> io::Result<OwnedFd> {
    // SAFETY: fd borrowed for the call; path is a valid C string.
    let ret = unsafe { libc::syscall(libc::SYS_fspick, fd_arg(dfd), path.as_ptr(), flags) };
    owned_fd(ret)
}

/// `mount_setattr(2)` — set per-mount attributes and/or propagation type on the
/// subtree at (`dfd`, `path`). `AT_RECURSIVE` in `flags` applies it to the whole
/// subtree atomically.
pub fn mount_setattr(
    dfd: BorrowedFd<'_>,
    path: &CStr,
    flags: u32,
    attr: &libc::mount_attr,
) -> io::Result<()> {
    // SAFETY: fd borrowed for the call; path valid; `attr` valid for
    // `size_of` bytes and only read by the kernel.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_mount_setattr,
            fd_arg(dfd),
            path.as_ptr(),
            flags,
            std::ptr::from_ref::<libc::mount_attr>(attr),
            size_of::<libc::mount_attr>(),
        )
    };
    check(ret)
}

/// `umount2(2)` — detach the mount at `target` with `flags` (`MNT_DETACH` /
/// `MNT_FORCE` / `UMOUNT_NOFOLLOW`).
pub fn umount2(target: &CStr, flags: i32) -> io::Result<()> {
    // SAFETY: `target` is a valid C string read only for the call.
    let ret = unsafe { libc::umount2(target.as_ptr(), flags) };
    check(ret as libc::c_long)
}

/// Drain the new-API `fs_context` message log off `fs_fd`. The kernel returns
/// one message per `read()`, prefixed by a class byte (`e`/`w`/`i`) and a
/// space. Per spec §12 the read buffer must be large enough to avoid
/// `EMSGSIZE` (which *consumes and discards* the message); we start at 4 KiB
/// and grow on `EMSGSIZE` so nothing is lost. Returns all lines, each retaining
/// its class prefix.
#[allow(clippy::comparison_chain)] // n: >0 message, ==0 drained, <0 error tail
pub fn drain_fs_log(fs_fd: BorrowedFd<'_>) -> Vec<String> {
    use std::os::fd::AsRawFd;
    let mut out = Vec::new();
    let mut cap = 4096usize;
    let mut buf = vec![0u8; cap];
    loop {
        // SAFETY: reading into an owned buffer of length `cap`.
        let n = unsafe {
            libc::read(
                fs_fd.as_raw_fd(),
                buf.as_mut_ptr().cast::<libc::c_void>(),
                cap,
            )
        };
        if n > 0 {
            out.push(String::from_utf8_lossy(&buf[..n as usize]).trim_end().to_string());
        } else if n == 0 {
            break;
        } else {
            let err = io::Error::last_os_error();
            // EMSGSIZE: the message didn't fit; grow and the *next* read gets
            // the following message (this one is already lost by the kernel,
            // but growing prevents losing any more).
            if err.raw_os_error() == Some(libc::EMSGSIZE) {
                cap = cap.saturating_mul(2).min(1 << 20);
                buf.resize(cap, 0);
                continue;
            }
            break; // EAGAIN / drained / other — stop.
        }
    }
    out
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// `libc::syscall` takes `c_long` fd arguments; centralise the cast.
fn fd_arg(fd: BorrowedFd<'_>) -> libc::c_long {
    use std::os::fd::AsRawFd;
    fd.as_raw_fd() as libc::c_long
}

/// Map a syscall return to `Result<()>`: negative ⇒ live errno.
fn check(ret: libc::c_long) -> io::Result<()> {
    if ret < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Map an fd-returning syscall to an owned fd, taking ownership of the result.
fn owned_fd(ret: libc::c_long) -> io::Result<OwnedFd> {
    if ret < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: the kernel just handed us a fresh, owned fd.
        Ok(unsafe { OwnedFd::from_raw_fd(ret as RawFd) })
    }
}
