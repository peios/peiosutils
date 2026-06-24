// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Loop-device setup (§5): reuse-first, auto-allocate via `LOOP_CTL_GET_FREE`,
//! attach via `LOOP_CONFIGURE` (fallback `LOOP_SET_FD`+`LOOP_SET_STATUS64`),
//! `LO_FLAGS_AUTOCLEAR` so the kernel frees the device on unmount, `EBUSY`
//! retry on the GET_FREE→CONFIGURE race, and detach-on-failure (§17).

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;

use crate::error::{MountError, Result};
use crate::options::{LoopRequest, ParsedOptions};

// loop ioctls (<linux/loop.h>).
const LOOP_SET_FD: libc::c_ulong = 0x4C00;
const LOOP_CLR_FD: libc::c_ulong = 0x4C01;
const LOOP_SET_STATUS64: libc::c_ulong = 0x4C04;
const LOOP_CONFIGURE: libc::c_ulong = 0x4C0A;
const LOOP_CTL_GET_FREE: libc::c_ulong = 0x4C82;

const LO_FLAGS_READ_ONLY: u32 = 1;
const LO_FLAGS_AUTOCLEAR: u32 = 4;

const MAX_GET_FREE_RETRIES: u32 = 16;

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(clippy::struct_field_names)] // mirrors the kernel's struct loop_info64
struct LoopInfo64 {
    lo_device: u64,
    lo_inode: u64,
    lo_rdevice: u64,
    lo_offset: u64,
    lo_sizelimit: u64,
    lo_number: u32,
    lo_encrypt_type: u32,
    lo_encrypt_key_size: u32,
    lo_flags: u32,
    lo_file_name: [u8; 64],
    lo_crypt_name: [u8; 64],
    lo_encrypt_key: [u8; 32],
    lo_init: [u64; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LoopConfig {
    fd: u32,
    block_size: u32,
    info: LoopInfo64,
    reserved: [u64; 8],
}

/// IO/verbosity context for loop operations.
#[derive(Clone, Copy)]
pub struct Io {
    pub verbose: u8,
    pub fake: bool,
}

impl Io {
    fn say(self, msg: impl FnOnce() -> String) {
        if self.verbose > 0 {
            eprintln!("mount: {}", msg());
        }
    }
}

/// A configured loop device. On drop before [`disarm`](LoopGuard::disarm) it
/// detaches the device (`LOOP_CLR_FD`) so an interrupted/failed flow leaks
/// nothing (§17). After a successful mount the caller disarms it — autoclear
/// then frees it on unmount.
pub struct LoopGuard {
    device: Vec<u8>,
    /// Held open only so `LOOP_CLR_FD` can run on drop.
    handle: Option<File>,
    armed: bool,
}

impl LoopGuard {
    pub fn device(&self) -> &[u8] {
        &self.device
    }

    /// Mark the mount successful: do not detach on drop (autoclear owns it).
    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for LoopGuard {
    fn drop(&mut self) {
        if self.armed {
            if let Some(h) = &self.handle {
                // SAFETY: LOOP_CLR_FD takes no argument; best-effort cleanup.
                unsafe { libc::ioctl(h.as_raw_fd(), LOOP_CLR_FD) };
            }
        }
    }
}

/// Set up a loop device if requested or implied (§5). Returns `None` when no
/// loop is needed.
pub fn maybe_setup(
    io: Io,
    source: &[u8],
    _fstype: &[u8],
    opts: &ParsedOptions,
) -> Result<Option<LoopGuard>> {
    if !decide(io, source, opts) {
        return Ok(None);
    }

    // Validation: offset/sizelimit are losetup options — meaningless on a
    // non-regular-file (block device) source.
    if (opts.offset.is_some() || opts.sizelimit.is_some()) && !is_regular_file(source) {
        return Err(MountError::Usage(
            "offset=/sizelimit= require a regular-file source (they are loop options)".to_string(),
        ));
    }

    // Reuse first (§5): an existing loop over the same file+offset+sizelimit.
    let offset = opts.offset.unwrap_or(0);
    let sizelimit = opts.sizelimit.unwrap_or(0);
    if let Some(dev) = find_existing(source, offset, sizelimit) {
        io.say(|| format!("reusing loop device {}", String::from_utf8_lossy(&dev)));
        if io.fake {
            return Ok(None);
        }
        return Ok(Some(LoopGuard { device: dev, handle: None, armed: false }));
    }

    if io.fake {
        io.say(|| format!("fake: would attach {} to a loop device", String::from_utf8_lossy(source)));
        return Ok(None);
    }

    // Explicit device, or auto-allocate.
    match &opts.loop_request {
        Some(LoopRequest::Device(dev)) => attach(io, dev.clone(), source, offset, sizelimit),
        _ => attach_auto(io, source, offset, sizelimit),
    }
}

/// Whether a loop device is wanted: explicit `loop`/`loop=`, `offset=`/
/// `sizelimit=` (imply loop), or implicit for a regular-file source (unless
/// `X-mount.noloop`).
fn decide(io: Io, source: &[u8], opts: &ParsedOptions) -> bool {
    if opts.loop_request.is_some() || opts.offset.is_some() || opts.sizelimit.is_some() {
        return true;
    }
    if opts.noloop {
        return false;
    }
    let implicit = is_regular_file(source);
    if implicit {
        io.say(|| "regular-file source: using an implicit loop device".to_string());
    }
    implicit
}

fn attach_auto(io: Io, source: &[u8], offset: u64, sizelimit: u64) -> Result<Option<LoopGuard>> {
    let control = File::open("/dev/loop-control")
        .map_err(|e| MountError::System(format!("opening /dev/loop-control: {e}")))?;
    let mut last_err = None;
    for _ in 0..MAX_GET_FREE_RETRIES {
        // SAFETY: LOOP_CTL_GET_FREE returns a free device number or <0.
        let n = unsafe { libc::ioctl(control.as_raw_fd(), LOOP_CTL_GET_FREE) };
        if n < 0 {
            return Err(MountError::from_syscall("LOOP_CTL_GET_FREE", std::io::Error::last_os_error()));
        }
        let dev = format!("/dev/loop{n}").into_bytes();
        match attach(io, dev, source, offset, sizelimit) {
            Ok(g) => return Ok(g),
            // The device was grabbed in the GET_FREE→CONFIGURE window — retry.
            Err(MountError::Mount { source: e, .. }) if e.raw_os_error() == Some(libc::EBUSY) => {
                last_err = Some(MountError::Mount { stage: "LOOP_CONFIGURE", source: e });
            }
            Err(other) => return Err(other),
        }
    }
    Err(last_err.unwrap_or_else(|| MountError::System("no free loop device".to_string())))
}

fn attach(
    io: Io,
    device: Vec<u8>,
    source: &[u8],
    offset: u64,
    sizelimit: u64,
) -> Result<Option<LoopGuard>> {
    let backing = OpenOptions::new()
        .read(true)
        .write(true)
        .open(os(source))
        .or_else(|_| OpenOptions::new().read(true).open(os(source)))
        .map_err(|e| MountError::from_syscall("open backing file", e))?;
    let read_only = backing.metadata().is_ok_and(|m| m.permissions().readonly());

    let loopdev = OpenOptions::new()
        .read(true)
        .write(true)
        .open(os(&device))
        .map_err(|e| MountError::from_syscall("open loop device", e))?;

    let mut info = zeroed_info();
    info.lo_offset = offset;
    info.lo_sizelimit = sizelimit;
    info.lo_flags = LO_FLAGS_AUTOCLEAR | if read_only { LO_FLAGS_READ_ONLY } else { 0 };
    let name_len = source.len().min(63);
    info.lo_file_name[..name_len].copy_from_slice(&source[..name_len]);

    let config = LoopConfig {
        fd: backing.as_raw_fd() as u32,
        block_size: 0,
        info,
        reserved: [0; 8],
    };

    // SAFETY: LOOP_CONFIGURE reads a LoopConfig; backing fd is valid.
    let rc = unsafe { libc::ioctl(loopdev.as_raw_fd(), LOOP_CONFIGURE, &config) };
    if rc < 0 {
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            // Old kernel without LOOP_CONFIGURE → SET_FD + SET_STATUS64.
            Some(libc::ENOTTY) | Some(libc::EINVAL) => {
                attach_legacy(&loopdev, &backing, &info)?;
            }
            _ => return Err(MountError::from_syscall("LOOP_CONFIGURE", err)),
        }
    }

    io.say(|| format!("attached {} to {}", String::from_utf8_lossy(source), String::from_utf8_lossy(&device)));
    Ok(Some(LoopGuard { device, handle: Some(loopdev), armed: true }))
}

fn attach_legacy(loopdev: &File, backing: &File, info: &LoopInfo64) -> Result<()> {
    // SAFETY: LOOP_SET_FD takes the backing fd as its argument.
    let rc = unsafe { libc::ioctl(loopdev.as_raw_fd(), LOOP_SET_FD, backing.as_raw_fd() as libc::c_long) };
    if rc < 0 {
        return Err(MountError::from_syscall("LOOP_SET_FD", std::io::Error::last_os_error()));
    }
    // SAFETY: LOOP_SET_STATUS64 reads a LoopInfo64.
    let rc = unsafe { libc::ioctl(loopdev.as_raw_fd(), LOOP_SET_STATUS64, info) };
    if rc < 0 {
        let err = std::io::Error::last_os_error();
        // SAFETY: undo the SET_FD on failure.
        unsafe { libc::ioctl(loopdev.as_raw_fd(), LOOP_CLR_FD) };
        return Err(MountError::from_syscall("LOOP_SET_STATUS64", err));
    }
    Ok(())
}

/// Scan `/sys/block/loop*` for a device already backing `source` at the same
/// offset and sizelimit (§5 corruption-avoidance reuse).
fn find_existing(source: &[u8], offset: u64, sizelimit: u64) -> Option<Vec<u8>> {
    let want = std::fs::canonicalize(os(source)).ok()?;
    let blocks = std::fs::read_dir("/sys/block").ok()?;
    for entry in blocks.flatten() {
        let name = entry.file_name();
        let name_bytes = name.as_bytes();
        if !name_bytes.starts_with(b"loop") || name_bytes.len() <= 4 {
            continue;
        }
        let base = entry.path().join("loop");
        let backing = read_trimmed(&base.join("backing_file"));
        let Some(backing) = backing else { continue };
        if std::fs::canonicalize(&backing).ok().as_deref() != Some(want.as_path()) {
            continue;
        }
        let dev_offset = read_trimmed(&base.join("offset"))
            .and_then(|s| s.to_str().and_then(|s| s.trim().parse::<u64>().ok()))
            .unwrap_or(0);
        let dev_sizelimit = read_trimmed(&base.join("sizelimit"))
            .and_then(|s| s.to_str().and_then(|s| s.trim().parse::<u64>().ok()))
            .unwrap_or(0);
        if dev_offset == offset && dev_sizelimit == sizelimit {
            let mut dev = b"/dev/".to_vec();
            dev.extend_from_slice(name_bytes);
            return Some(dev);
        }
    }
    None
}

fn read_trimmed(p: &std::path::Path) -> Option<std::ffi::OsString> {
    let data = std::fs::read(p).ok()?;
    let trimmed = data.strip_suffix(b"\n").unwrap_or(&data);
    Some(std::ffi::OsStr::from_bytes(trimmed).to_os_string())
}

fn is_regular_file(source: &[u8]) -> bool {
    std::fs::metadata(os(source))
        .is_ok_and(|m| {
            let ft = m.file_type();
            !ft.is_block_device() && !ft.is_char_device() && m.is_file()
        })
}

fn os(b: &[u8]) -> &std::ffi::OsStr {
    std::ffi::OsStr::from_bytes(b)
}

fn zeroed_info() -> LoopInfo64 {
    // SAFETY: LoopInfo64 is plain old data; all-zero is a valid value.
    unsafe { std::mem::zeroed() }
}

/// `umount -d` (§5.1): best-effort, verified detach of a loop device. Only
/// clears the device if it still has a backing file (so a number recycled by a
/// racing mount is left alone). Mount-created loops carry `LO_FLAGS_AUTOCLEAR`
/// and free themselves on unmount, so this is normally redundant; it force-
/// clears loops that lack autoclear.
pub fn detach_verified(device: &[u8]) -> Result<()> {
    // Verify the device still backs something (a recycled-but-free slot has no
    // backing_file); if it's already gone, treat as a no-op.
    let dev_str = String::from_utf8_lossy(device);
    let name = dev_str.rsplit('/').next().unwrap_or(&dev_str);
    let backing = std::path::Path::new("/sys/block").join(name).join("loop/backing_file");
    if read_trimmed(&backing).is_none() {
        return Ok(()); // not currently backing anything — nothing to clear
    }
    let f = File::options()
        .read(true)
        .write(true)
        .open(os(device))
        .map_err(|e| MountError::from_syscall("open loop device", e))?;
    // SAFETY: LOOP_CLR_FD takes no argument.
    let rc = unsafe { libc::ioctl(f.as_raw_fd(), LOOP_CLR_FD) };
    if rc < 0 {
        return Err(MountError::from_syscall("LOOP_CLR_FD", std::io::Error::last_os_error()));
    }
    Ok(())
}
