// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! The verb executors: turn a [`MountRequest`] into the new-API syscall
//! sequence (§2.1). Each verb composes `sys` primitives; propagation changes
//! (§4.7) run as trailing `mount_setattr` steps after the primary operation.
//!
//! Loop setup (§5), libblkid source/type resolution (§7) and the KACS policy
//! step (§8) are delegated to their modules and woven in here.

use std::ffi::{CString, OsStr};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;

use crate::error::{MountError, Result};
use crate::options::{PropChange, PropKind, SbFlag};
use crate::request::MountRequest;
use crate::verb::Verb;
use crate::{blkid, loopdev, policy, sys};

/// Execution context: verbose narration + dry-run flag.
struct Ctx {
    verbose: u8,
    fake: bool,
}

impl Ctx {
    fn say(&self, msg: impl FnOnce() -> String) {
        if self.verbose > 0 {
            eprintln!("mount: {}", msg());
        }
    }
}

/// Execute a fully-resolved mount request.
pub fn execute(req: &MountRequest) -> Result<()> {
    let ctx = Ctx { verbose: req.verbose, fake: req.fake };

    // Canonicalize the target in the origin namespace (§13).
    let target = canonicalize(&req.target, req.no_canonicalize_target, req.mkdir)?;

    // Namespaces (§13): each verb does source/UUID resolution + canonicalization
    // in the ORIGIN ns, then `setns()` into the target ns immediately before the
    // attaching/mutating syscall (`enter_ns`). pkm ns authz is immature, so
    // residual cross-ns clumsiness is the substrate's, not ours (§1.5).

    // X-mount.subdir is effective only for a new instance; silently ignored
    // for the other verbs (§6.9) — surface the ignore under -v.
    if req.options.subdir.is_some() && req.verb != Verb::New {
        ctx.say(|| "X-mount.subdir is ignored for bind/move/remount/propagation".to_string());
    }

    match req.verb {
        Verb::New => exec_new(&ctx, req, &target)?,
        Verb::Bind { recursive } => exec_bind(&ctx, req, &target, recursive)?,
        Verb::Move => exec_move(&ctx, req, &target, 0)?,
        Verb::Beneath => exec_move(&ctx, req, &target, libc::MOVE_MOUNT_BENEATH)?,
        Verb::Remount => exec_remount(&ctx, req, &target)?,
    }

    // Trailing propagation changes (§4.7 / §17): applied in order; a failure
    // here leaves the mount in place (it is valid) and is reported separately.
    apply_propagation(&ctx, &target, &req.propagation)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// New mount
// ---------------------------------------------------------------------------

fn exec_new(ctx: &Ctx, req: &MountRequest, target: &[u8]) -> Result<()> {
    let o = &req.options;

    // Source resolution (§7): tags (-U/-L/UUID=…) → device via libblkid.
    let raw_source = req
        .source
        .clone()
        .ok_or_else(|| MountError::Usage("a source is required for a new mount".to_string()))?;
    let resolved = blkid::resolve_source(&raw_source)?;

    // Filesystem type: explicit, or probed (§7) when omitted/auto. A pseudo /
    // sourceless source (`none`, or a non-path spec) cannot be probed — require
    // an explicit -t with a clean message rather than a confusing probe error
    // (§2.3/§7).
    let fstype = match &req.fstype {
        Some(t) => t.clone(),
        None if resolved == b"none" || !resolved.starts_with(b"/") => {
            return Err(MountError::Usage(format!(
                "a filesystem type is required (-t) for the source '{}' (it cannot be probed)",
                String::from_utf8_lossy(&resolved)
            )));
        }
        None => blkid::probe_type(&resolved, o.auto_fstypes.as_deref())?,
    };
    reject_swap(&fstype)?;

    // Loop setup (§5): explicit `-o loop`/`loop=`, or implicit for a regular
    // file recognised by libblkid (unless X-mount.noloop). Owned so the guard
    // can be disarmed after a successful mount.
    let mut loop_guard = loopdev::maybe_setup(ctx_io(ctx), &resolved, &fstype, o)?;
    let device: Vec<u8> = loop_guard
        .as_ref()
        .map_or_else(|| resolved.clone(), |l| l.device().to_vec());

    ctx.say(|| {
        format!(
            "mounting {} (type {}) at {}",
            String::from_utf8_lossy(&device),
            String::from_utf8_lossy(&fstype),
            String::from_utf8_lossy(target)
        )
    });

    if ctx.fake {
        ctx.say(|| "fake: skipping mount syscalls".to_string());
        return Ok(());
    }

    // Auto-ro fallback (§2.5.1): retry read-only on a write-protect failure
    // unless rw was explicitly requested.
    let attempt = |force_ro: bool| -> Result<()> {
        let detached = build_new_fsmount(req, &device, &fstype, force_ro)?;
        // KACS policy set-before-attach (§8.3): on the DETACHED fd, before
        // move_mount. On failure the fd is dropped and nothing is attached.
        // (Set on the whole-fs mount so a subdir clone shares the sb policy.)
        policy::apply_set_before_attach(detached.as_fd(), o.policy, req.synth_sddl.as_deref())?;
        // The superblock was created from the origin-resolved device; enter the
        // target ns now, just before attaching (§13).
        enter_ns(req)?;
        let tgt = cstr(target)?;

        // X-mount.subdir (§6.9): attach a subdirectory of the fresh instance.
        // Clone the subtree rooted at DIR within the detached mount, then graft
        // the clone. Only meaningful for a new instance (here).
        if let Some(subdir) = &o.subdir {
            let rel = cstr(subdir.strip_prefix(b"/").unwrap_or(subdir))?;
            let clone = sys::open_tree(
                detached.as_fd(),
                &rel,
                sys::OPEN_TREE_CLONE | sys::OPEN_TREE_CLOEXEC,
            )
            .map_err(MountError::at("open_tree(subdir)"))?;
            return sys::move_mount_attach(clone.as_fd(), &tgt, move_mount_flags(req))
                .map_err(MountError::at("move_mount"));
        }

        sys::move_mount_attach(detached.as_fd(), &tgt, move_mount_flags(req))
            .map_err(MountError::at("move_mount"))
    };

    let result = match attempt(false) {
        Ok(()) => Ok(()),
        Err(e) if should_retry_ro(req, &e) => {
            ctx.say(|| "write-protected; retrying read-only".to_string());
            attempt(true)
        }
        Err(e) => Err(e),
    };
    // On success the loop device stays (autoclear frees it on unmount); on
    // failure the guard's Drop detaches it (§17).
    if result.is_ok() {
        if let Some(g) = loop_guard.as_mut() {
            g.disarm();
        }
    }
    result
}

/// Build the detached `fsmount` fd: `fsopen → fsconfig(source, params, sb) →
/// CMD_CREATE[_EXCL] → fsmount`.
fn build_new_fsmount(
    req: &MountRequest,
    device: &[u8],
    fstype: &[u8],
    force_ro: bool,
) -> Result<OwnedFd> {
    let o = &req.options;
    let fst = cstr(fstype)?;
    let fs_fd = sys::fsopen(&fst, sys::FSOPEN_CLOEXEC).map_err(MountError::at("fsopen"))?;

    // source=<device> unless this is a pseudo/sourceless fs spelled `none`.
    if device != b"none" {
        set_string(fs_fd.as_fd(), b"source", device)?;
    }
    // Superblock-level options for a fresh mount go through fsconfig.
    for (flag, on) in &o.sb_flags {
        set_flag(fs_fd.as_fd(), sb_flag_key(*flag, *on))?;
    }
    if o.sb_rdonly == Some(true) || force_ro {
        set_flag(fs_fd.as_fd(), b"ro")?;
    }
    // fs-specific opaque params (Category C).
    for (key, value) in &o.fs_params {
        match value {
            Some(v) => set_string(fs_fd.as_fd(), key, v)?,
            None => set_flag(fs_fd.as_fd(), key)?,
        }
    }

    let create = if req.exclusive {
        sys::fsconfig_cmd::CMD_CREATE_EXCL
    } else {
        sys::fsconfig_cmd::CMD_CREATE
    };
    sys::fsconfig_cmd(fs_fd.as_fd(), create)
        .map_err(|e| with_log("fsconfig(create)", fs_fd.as_fd(), e))?;

    let mut attr = o.fsmount_attr_flags();
    if force_ro {
        attr |= libc::MOUNT_ATTR_RDONLY as u32;
    }
    sys::fsmount(fs_fd.as_fd(), sys::FSMOUNT_CLOEXEC, attr).map_err(MountError::at("fsmount"))
}

// ---------------------------------------------------------------------------
// Bind / move / beneath
// ---------------------------------------------------------------------------

fn exec_bind(ctx: &Ctx, req: &MountRequest, target: &[u8], recursive: bool) -> Result<()> {
    let o = &req.options;
    // A bind shares the source superblock; sb-level options are invalid (§6.7).
    if o.touches_superblock() {
        return Err(MountError::Usage(
            "superblock options (fs params / sync / ro=fs / …) are not valid on a bind; \
             only per-mount VFS attrs are"
                .to_string(),
        ));
    }
    let source = req
        .source
        .clone()
        .ok_or_else(|| MountError::Usage("a source is required for a bind".to_string()))?;
    let source = canonicalize(&source, req.no_canonicalize_source, None)?;

    ctx.say(|| {
        format!(
            "{}binding {} to {}",
            if recursive { "recursively " } else { "" },
            String::from_utf8_lossy(&source),
            String::from_utf8_lossy(target)
        )
    });
    if ctx.fake {
        return Ok(());
    }

    let src = cstr(&source)?;
    let mut flags = sys::OPEN_TREE_CLONE | sys::OPEN_TREE_CLOEXEC;
    if recursive {
        flags |= sys::AT_RECURSIVE;
    }
    // open_tree reads the source subtree in the origin ns.
    let clone = sys::open_tree(at_fdcwd(), &src, flags).map_err(MountError::at("open_tree"))?;

    // Apply per-mount attrs to the (detached) clone (§6.8) if any were requested.
    if o.attr_set != 0 || o.attr_clr != 0 {
        let mut sa_flags = sys::AT_EMPTY_PATH;
        if o.recursive_attr {
            sa_flags |= sys::AT_RECURSIVE;
        }
        let attr = mount_attr(o.attr_set, o.attr_clr, 0);
        sys::mount_setattr(clone.as_fd(), c"", sa_flags, &attr)
            .map_err(MountError::at("mount_setattr"))?;
    }

    // Enter the target ns just before grafting (§13).
    enter_ns(req)?;
    let tgt = cstr(target)?;
    sys::move_mount_attach(clone.as_fd(), &tgt, move_mount_flags(req)).map_err(MountError::at("move_mount"))
}

fn exec_move(ctx: &Ctx, req: &MountRequest, target: &[u8], flags: u32) -> Result<()> {
    let source = req
        .source
        .clone()
        .ok_or_else(|| MountError::Usage("a source mount is required for move".to_string()))?;
    let source = canonicalize(&source, req.no_canonicalize_source, None)?;
    ctx.say(|| {
        format!(
            "moving {} to {}{}",
            String::from_utf8_lossy(&source),
            String::from_utf8_lossy(target),
            if flags & libc::MOVE_MOUNT_BENEATH != 0 { " (beneath)" } else { "" }
        )
    });
    if ctx.fake {
        return Ok(());
    }
    enter_ns(req)?; // source canonicalized in origin above; relocate in target ns
    let src = cstr(&source)?;
    let tgt = cstr(target)?;
    sys::move_mount_path(&src, &tgt, flags).map_err(MountError::at("move_mount"))
}

// ---------------------------------------------------------------------------
// Remount (§6.7, per-layer)
// ---------------------------------------------------------------------------

fn exec_remount(ctx: &Ctx, req: &MountRequest, target: &[u8]) -> Result<()> {
    let o = &req.options;

    // Enter the target ns first (§13): the mount being reconfigured — and the
    // mountinfo the bind guard consults — live in that ns. (No-op under --fake.)
    enter_ns(req)?;

    // Shared-superblock remount guard (§6.7): a mount whose fs-root is not `/`
    // (a bind, or a subvolume) shares its superblock with other mounts of the
    // same filesystem, so a superblock-level option on its remount would mutate
    // all of them. This is a deliberate userspace policy guard (stricter than
    // util-linux, per §6.7) — refuse it; only per-mount VFS attrs are allowed.
    if o.touches_superblock() && shares_superblock(target) {
        return Err(MountError::Usage(
            "superblock-level options are not valid when remounting a bind or subvolume \
             (a shared superblock) — they would affect every mount of that filesystem; \
             only per-mount VFS attributes (ro/nosuid/… or =recursive) are allowed here"
                .to_string(),
        ));
    }

    if ctx.fake {
        ctx.say(|| format!("fake: would remount {}", String::from_utf8_lossy(target)));
        return Ok(());
    }

    // Per-mount (VFS) layer: mount_setattr with the attr mask — only named bits
    // change, no read-and-preserve.
    if o.attr_set != 0 || o.attr_clr != 0 {
        let mut flags = 0u32;
        if o.recursive_attr {
            flags |= sys::AT_RECURSIVE;
        }
        let attr = mount_attr(o.attr_set, o.attr_clr, 0);
        let tgt = cstr(target)?;
        sys::mount_setattr(at_fdcwd(), &tgt, flags, &attr)
            .map_err(MountError::at("mount_setattr"))?;
        ctx.say(|| "applied per-mount attribute changes".to_string());
    }

    // Superblock / fs-param layer: fspick + fsconfig(RECONFIGURE).
    if o.touches_superblock() {
        let tgt = cstr(target)?;
        let fs_fd = sys::fspick(at_fdcwd(), &tgt, sys::fspick::CLOEXEC)
            .map_err(MountError::at("fspick"))?;
        for (flag, on) in &o.sb_flags {
            set_flag(fs_fd.as_fd(), sb_flag_key(*flag, *on))?;
        }
        if let Some(ro) = o.sb_rdonly {
            set_flag(fs_fd.as_fd(), if ro { b"ro" } else { b"rw" })?;
        }
        for (key, value) in &o.fs_params {
            match value {
                Some(v) => set_string(fs_fd.as_fd(), key, v)?,
                None => set_flag(fs_fd.as_fd(), key)?,
            }
        }
        sys::fsconfig_cmd(fs_fd.as_fd(), sys::fsconfig_cmd::CMD_RECONFIGURE)
            .map_err(|e| with_log("fsconfig(reconfigure)", fs_fd.as_fd(), e))?;
        ctx.say(|| "reconfigured superblock options".to_string());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Propagation (§2.1 / §4.7)
// ---------------------------------------------------------------------------

fn apply_propagation(ctx: &Ctx, target: &[u8], changes: &[PropChange]) -> Result<()> {
    for ch in changes {
        ctx.say(|| format!("setting propagation {:?}{}", ch.kind, if ch.recursive { " (recursive)" } else { "" }));
        if ctx.fake {
            continue;
        }
        let prop = match ch.kind {
            PropKind::Shared => sys::propagation::SHARED,
            PropKind::Slave => sys::propagation::SLAVE,
            PropKind::Private => sys::propagation::PRIVATE,
            PropKind::Unbindable => sys::propagation::UNBINDABLE,
        };
        let flags = if ch.recursive { sys::AT_RECURSIVE } else { 0 };
        let attr = mount_attr(0, 0, prop);
        let tgt = cstr(target)?;
        sys::mount_setattr(at_fdcwd(), &tgt, flags, &attr)
            .map_err(MountError::at("mount_setattr(propagation)"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn ctx_io(ctx: &Ctx) -> loopdev::Io {
    loopdev::Io { verbose: ctx.verbose, fake: ctx.fake }
}

fn move_mount_flags(req: &MountRequest) -> u32 {
    // TOCTOU/no-follow (§4.1/§17): `move_mount` does NOT follow a trailing
    // symlink on the target unless MOVE_MOUNT_T_SYMLINKS is set — so the safe
    // no-follow posture is the default (0). We never opt into following.
    let _ = req;
    0
}

fn should_retry_ro(req: &MountRequest, e: &MountError) -> bool {
    if req.options.explicit_rw || req.options.explicit_ro || req.options.sb_rdonly == Some(true) {
        return false;
    }
    // Write-protect surfaces as EROFS, or as EACCES from the block/sb layer.
    matches!(
        e,
        MountError::Mount { source, .. } | MountError::Permission { source, .. }
            if matches!(source.raw_os_error(), Some(libc::EROFS) | Some(libc::EACCES))
    )
}

fn sb_flag_key(flag: SbFlag, on: bool) -> &'static [u8] {
    match (flag, on) {
        (SbFlag::Synchronous, true) => b"sync",
        (SbFlag::Synchronous, false) => b"async",
        (SbFlag::Dirsync, _) => b"dirsync",
        (SbFlag::Lazytime, true) => b"lazytime",
        (SbFlag::Lazytime, false) => b"nolazytime",
        (SbFlag::IVersion, true) => b"iversion",
        (SbFlag::IVersion, false) => b"noiversion",
        (SbFlag::Silent, true) => b"silent",
        (SbFlag::Silent, false) => b"loud",
    }
}

fn reject_swap(fstype: &[u8]) -> Result<()> {
    if fstype == b"swap" {
        return Err(MountError::Usage(
            "'swap' is not a mountable filesystem — use swapon/swapoff".to_string(),
        ));
    }
    Ok(())
}

/// Whether the mount at `target` shares its superblock with other mounts (its
/// fs-root is not `/`): a bind or a subvolume. See the §6.7 guard.
fn shares_superblock(target: &[u8]) -> bool {
    match crate::mountinfo::read() {
        Ok(entries) => crate::mountinfo::at_mountpoint(&entries, target)
            .is_some_and(super::mountinfo::MountEntry::is_bind),
        Err(_) => false,
    }
}

fn mount_attr(attr_set: u64, attr_clr: u64, propagation: u64) -> libc::mount_attr {
    libc::mount_attr { attr_set, attr_clr, propagation, userns_fd: 0 }
}

fn set_string(fs_fd: BorrowedFd<'_>, key: &[u8], value: &[u8]) -> Result<()> {
    let k = cstr(key)?;
    let v = cstr(value)?;
    sys::fsconfig_set_string(fs_fd, &k, &v).map_err(|e| with_log("fsconfig", fs_fd, e))
}

fn set_flag(fs_fd: BorrowedFd<'_>, key: &[u8]) -> Result<()> {
    let k = cstr(key)?;
    sys::fsconfig_set_flag(fs_fd, &k).map_err(|e| with_log("fsconfig", fs_fd, e))
}

/// Build a `CString`, rejecting interior NUL as a usage error (§1.8).
fn cstr(s: &[u8]) -> Result<CString> {
    CString::new(s).map_err(|_| MountError::Usage("argument contains a NUL byte".to_string()))
}

/// Canonicalize a path (absolute + symlink-resolved) unless suppressed; create
/// it first if `mkdir` is requested. Falls back to the given bytes when the
/// path doesn't exist yet and isn't being created.
// Returns Result for API stability: the no-follow resolution path (§4.1/§17)
// will surface real errors here.
#[allow(clippy::unnecessary_wraps)]
fn canonicalize(path: &[u8], no_canon: bool, mkdir: Option<u32>) -> Result<Vec<u8>> {
    let p = OsStr::from_bytes(path);
    if let Some(mode) = mkdir {
        let _ = std::fs::create_dir_all(p); // best-effort; mount surfaces real errors
        let _ = mode; // mode application is best-effort via umask; see §6.9
    }
    if no_canon {
        return Ok(path.to_vec());
    }
    match std::fs::canonicalize(p) {
        Ok(real) => Ok(real.as_os_str().as_bytes().to_vec()),
        Err(_) => Ok(path.to_vec()),
    }
}

/// Drain the fs_context log and fold any kernel error message into the error so
/// a config failure names the offending option in the kernel's words (§12: the
/// log is reported on failure regardless of `-v`).
fn with_log(stage: &'static str, fs_fd: BorrowedFd<'_>, source: std::io::Error) -> MountError {
    let msgs = sys::drain_fs_log(fs_fd);
    let errors: Vec<&str> = msgs
        .iter()
        .filter_map(|m| m.strip_prefix("e "))
        .collect();
    if errors.is_empty() {
        MountError::from_syscall(stage, source)
    } else {
        let detail = errors.join("; ");
        MountError::from_syscall(
            stage,
            std::io::Error::new(source.kind(), format!("{detail} ({source})")),
        )
    }
}

/// Enter the target mount namespace (`-N`) if one was requested, just before an
/// attaching/mutating syscall (§13). No-op under `--fake` or without `-N`.
fn enter_ns(req: &MountRequest) -> Result<()> {
    match &req.namespace {
        Some(ns) if !req.fake => crate::namespace::enter(ns),
        _ => Ok(()),
    }
}

/// `AT_FDCWD` as a `BorrowedFd`, for the path-taking syscalls
/// (`open_tree`/`fspick`/`mount_setattr` on an absolute path).
fn at_fdcwd() -> BorrowedFd<'static> {
    // SAFETY: AT_FDCWD is a well-known special dirfd the *at syscalls accept;
    // it is never closed.
    unsafe { BorrowedFd::borrow_raw(libc::AT_FDCWD) }
}
