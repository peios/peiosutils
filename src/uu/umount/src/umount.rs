// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! umount ~ (peiosutils) — detach Peios filesystems via `umount2(2)` (§2.6).
//!
//! A thin CLI over the shared `pu_mount` library core. Resolves each operand to
//! a mount point (or, for a source, its mount point(s) via live mountinfo,
//! §2.6.2), composes `-R` (recursive, incl. over-mount stacks) and `-A` (all
//! targets of a source), maps flags to `umount2`, and accumulates exit codes
//! (§10: single `-R`/`-A` stops on first failure → 32; several source args
//! mixing success and failure → 64).

use std::ffi::{CString, OsStr};
use std::os::unix::ffi::OsStrExt;

use clap::{Arg, ArgAction, Command};
use uucore::error::{UResult, USimpleError};

use pu_mount::error::{MountError, Result};
use pu_mount::mountinfo::{self, MountEntry};
use pu_mount::{loopdev, sys};

mod opt {
    pub const LAZY: &str = "lazy";
    pub const FORCE: &str = "force";
    pub const RECURSIVE: &str = "recursive";
    pub const ALL_TARGETS: &str = "all-targets";
    pub const DETACH_LOOP: &str = "detach-loop";
    pub const READ_ONLY: &str = "read-only";
    pub const GRACEFUL: &str = "graceful";
    pub const QUIET: &str = "quiet";
    pub const NO_CANONICALIZE: &str = "no-canonicalize";
    pub const VERBOSE: &str = "verbose";
    pub const FAKE: &str = "fake";
    pub const NAMESPACE: &str = "namespace";
    pub const NO_MTAB: &str = "no-mtab";
    pub const INTERNAL_ONLY: &str = "internal-only";
    pub const OPERANDS: &str = "operands";
}

#[derive(Clone, Copy)]
struct Flags {
    lazy: bool,
    force: bool,
    recursive: bool,
    all_targets: bool,
    detach_loop: bool,
    read_only: bool,
    graceful: bool,
    quiet: bool,
    no_canonicalize: bool,
    verbose: u8,
    fake: bool,
}

/// The mount namespace to operate in (`-N`), if any.
fn namespace_arg(m: &clap::ArgMatches) -> Option<Vec<u8>> {
    m.get_one::<std::ffi::OsString>(opt::NAMESPACE)
        .map(|s| s.as_bytes().to_vec())
}

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match build().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            // clap returns 2 for argument errors; util-linux umount uses exit 1
            // (§10). Help/version stay 0.
            e.print().ok();
            return if e.use_stderr() { Err(USimpleError::new(1, "")) } else { Ok(()) };
        }
    };
    run(&matches).map_err(|e| USimpleError::new(e.exit_code(), e.to_string()))
}

pub fn uu_app() -> Command {
    build()
}

fn run(m: &clap::ArgMatches) -> Result<()> {
    let flags = Flags {
        lazy: m.get_flag(opt::LAZY),
        force: m.get_flag(opt::FORCE),
        recursive: m.get_flag(opt::RECURSIVE),
        all_targets: m.get_flag(opt::ALL_TARGETS),
        detach_loop: m.get_flag(opt::DETACH_LOOP),
        read_only: m.get_flag(opt::READ_ONLY),
        graceful: m.get_flag(opt::GRACEFUL),
        quiet: m.get_flag(opt::QUIET),
        no_canonicalize: m.get_flag(opt::NO_CANONICALIZE),
        verbose: m.get_count(opt::VERBOSE),
        fake: m.get_flag(opt::FAKE),
    };

    // -R and -r are mutually exclusive (util-linux excl[] guard).
    if flags.recursive && flags.read_only {
        return Err(MountError::Usage(
            "--recursive and --read-only are mutually exclusive".to_string(),
        ));
    }

    let operands: Vec<Vec<u8>> = m
        .get_many::<std::ffi::OsString>(opt::OPERANDS)
        .map(|vs| vs.map(|s| s.as_bytes().to_vec()).collect())
        .unwrap_or_default();
    if operands.is_empty() {
        return Err(MountError::Usage("a target or source is required".to_string()));
    }

    // -N (§13): enter the target mount ns before reading its mountinfo and
    // unmounting — umount operates entirely within that namespace. (Skip the
    // real setns under --fake.)
    if let Some(ns) = namespace_arg(m) {
        if !flags.fake {
            pu_mount::namespace::enter(&ns)?;
        }
    }

    // Accumulate across operands (§10): mixed success/failure → exit 64.
    let entries = mountinfo::read().map_err(|e| MountError::System(format!("mountinfo: {e}")))?;
    let mut any_ok = false;
    let mut first_err: Option<MountError> = None;
    for operand in &operands {
        match unmount_operand(operand, &entries, flags) {
            Ok(()) => any_ok = true,
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    match first_err {
        None => Ok(()),
        Some(e) if any_ok => Err(MountError::Partial(format!(
            "some unmounts failed: {e}"
        ))),
        Some(e) => Err(e),
    }
}

/// Resolve one operand to its target mount point(s) and unmount them, composing
/// `-R`/`-A`. Stop-on-first-failure within this operand.
fn unmount_operand(operand: &[u8], entries: &[MountEntry], flags: Flags) -> Result<()> {
    let canon = canonicalize(operand, flags.no_canonicalize);
    let targets = resolve_targets(&canon, operand, entries, flags)?;

    if targets.is_empty() {
        // Nothing matched: not mounted / absent.
        if flags.graceful {
            return Ok(());
        }
        let msg = if flags.quiet {
            String::new()
        } else {
            format!("{}: not mounted", String::from_utf8_lossy(operand))
        };
        return Err(MountError::NotMounted(msg));
    }

    for target in targets {
        let mut to_unmount = vec![target.clone()];
        if flags.recursive {
            to_unmount = recursive_order(&target, entries);
        }
        for mp in to_unmount {
            unmount_one(&mp, entries, flags)?;
        }
    }
    Ok(())
}

/// Map an operand to the mount point(s) it designates (§2.6.2). A path that is
/// itself a mount point is a target; otherwise it is treated as a source and
/// matched against mountinfo (ambiguous → error unless `-A`).
fn resolve_targets(
    canon: &[u8],
    raw: &[u8],
    entries: &[MountEntry],
    flags: Flags,
) -> Result<Vec<Vec<u8>>> {
    // A mount point (possibly an over-mount stack — dedup to one entry here;
    // recursion/stack-popping happens at unmount time).
    if entries.iter().any(|e| e.mount_point == canon) {
        return Ok(vec![canon.to_vec()]);
    }
    // Otherwise a source spec.
    let mut points: Vec<Vec<u8>> = Vec::new();
    for e in entries {
        if (e.source == raw || e.source == canon) && !points.contains(&e.mount_point) {
            points.push(e.mount_point.clone());
        }
    }
    match points.len() {
        0 => Ok(Vec::new()),
        1 => Ok(points),
        _ if flags.all_targets => Ok(points),
        _ => Err(MountError::Usage(format!(
            "{} is mounted at multiple places; specify a target or use -A:\n  {}",
            String::from_utf8_lossy(raw),
            points.iter().map(|p| String::from_utf8_lossy(p).into_owned()).collect::<Vec<_>>().join("\n  ")
        ))),
    }
}

/// Mount points to unmount for `-R` under `target`: the target, its over-mount
/// stack, and every descendant mount, ordered deepest-first.
fn recursive_order(target: &[u8], entries: &[MountEntry]) -> Vec<Vec<u8>> {
    let mut prefix = target.to_vec();
    if !prefix.ends_with(b"/") {
        prefix.push(b'/');
    }
    let mut points: Vec<Vec<u8>> = Vec::new();
    for e in entries {
        let under = e.mount_point == target || e.mount_point.starts_with(&prefix);
        if under {
            points.push(e.mount_point.clone());
        }
    }
    // Deepest (longest path) first; preserves over-mount stack order at a point.
    points.sort_by_key(|p| std::cmp::Reverse(p.len()));
    points
}

/// Unmount a single mount point, applying `umount2` flags and the `-r`/`-d`
/// behaviours. `-l/--lazy`→`MNT_DETACH`; `-f`→`MNT_FORCE`; `-c`→
/// `UMOUNT_NOFOLLOW`.
fn unmount_one(mp: &[u8], entries: &[MountEntry], flags: Flags) -> Result<()> {
    if flags.verbose > 0 {
        eprintln!("umount: unmounting {}", String::from_utf8_lossy(mp));
    }
    if flags.fake {
        return Ok(());
    }

    let mut umount_flags = 0;
    if flags.lazy {
        umount_flags |= sys::MNT_DETACH;
    }
    if flags.force {
        umount_flags |= sys::MNT_FORCE;
    }
    if flags.no_canonicalize {
        umount_flags |= sys::UMOUNT_NOFOLLOW;
    }

    let tgt = cstr(mp)?;
    match sys::umount2(&tgt, umount_flags) {
        Ok(()) => {
            // -d (§5.1): detach a loop device backing this mount, best-effort.
            if flags.detach_loop {
                if let Some(dev) = loop_source(mp, entries) {
                    let _ = loopdev::detach_verified(&dev);
                }
            }
            Ok(())
        }
        Err(e) => {
            // -r: on failure, try to remount read-only instead (§6.7).
            if flags.read_only {
                if flags.verbose > 0 {
                    eprintln!("umount: {}: remounting read-only", String::from_utf8_lossy(mp));
                }
                if remount_ro(mp).is_ok() {
                    return Ok(());
                }
            }
            if flags.graceful && e.raw_os_error() == Some(libc::EINVAL) {
                return Ok(());
            }
            Err(MountError::from_syscall("umount2", e))
        }
    }
}

/// The `/dev/loopN` source backing a mount point, if any.
fn loop_source(mp: &[u8], entries: &[MountEntry]) -> Option<Vec<u8>> {
    entries
        .iter()
        .find(|e| e.mount_point == mp)
        .map(|e| e.source.clone())
        .filter(|s| s.starts_with(b"/dev/loop"))
}

/// Remount a mount point read-only via `mount_setattr` (the `-r` fallback).
fn remount_ro(mp: &[u8]) -> Result<()> {
    let tgt = cstr(mp)?;
    let attr = libc::mount_attr {
        attr_set: libc::MOUNT_ATTR_RDONLY,
        attr_clr: 0,
        propagation: 0,
        userns_fd: 0,
    };
    // SAFETY: AT_FDCWD is a valid special dirfd for the *at syscalls.
    let dirfd = unsafe { std::os::fd::BorrowedFd::borrow_raw(libc::AT_FDCWD) };
    sys::mount_setattr(dirfd, &tgt, 0, &attr).map_err(MountError::at("mount_setattr"))
}

fn canonicalize(operand: &[u8], no_canon: bool) -> Vec<u8> {
    if no_canon {
        return operand.to_vec();
    }
    std::fs::canonicalize(OsStr::from_bytes(operand))
        .map_or_else(|_| operand.to_vec(), |p| p.as_os_str().as_bytes().to_vec())
}

fn cstr(s: &[u8]) -> Result<CString> {
    CString::new(s).map_err(|_| MountError::Usage("argument contains a NUL byte".to_string()))
}

fn boolean(name: &'static str) -> Arg {
    Arg::new(name).action(ArgAction::SetTrue)
}

fn build() -> Command {
    Command::new("umount")
        .version(uucore::crate_version!())
        .about("Detach a filesystem from the Peios mount tree")
        .arg(boolean(opt::LAZY).short('l').long("lazy").help("Detach now, clean up later (MNT_DETACH)"))
        .arg(boolean(opt::FORCE).short('f').long("force").help("Force unmount (MNT_FORCE)"))
        .arg(boolean(opt::RECURSIVE).short('R').long("recursive").help("Unmount the target and everything under it"))
        .arg(boolean(opt::ALL_TARGETS).short('A').long("all-targets").help("Unmount all mountpoints of the given source"))
        .arg(boolean(opt::DETACH_LOOP).short('d').long("detach-loop").help("Free the backing loop device after unmount"))
        .arg(boolean(opt::READ_ONLY).short('r').long("read-only").help("On failure, remount read-only instead"))
        .arg(boolean(opt::GRACEFUL).short('g').long("graceful").help("Exit 0 if the target is not mounted"))
        .arg(boolean(opt::QUIET).short('q').long("quiet").help("Suppress 'not mounted' messages"))
        .arg(boolean(opt::NO_CANONICALIZE).short('c').long("no-canonicalize").help("Do not canonicalize paths (UMOUNT_NOFOLLOW)"))
        .arg(Arg::new(opt::VERBOSE).short('v').long("verbose").action(ArgAction::Count).help("Say what is being done"))
        .arg(boolean(opt::FAKE).long("fake").help("Dry run: skip the unmount syscalls"))
        .arg(Arg::new(opt::NAMESPACE).short('N').long("namespace").value_name("NS").value_parser(clap::value_parser!(std::ffi::OsString)).help("Operate in mount namespace NS"))
        .arg(boolean(opt::NO_MTAB).short('n').long("no-mtab").help("Accepted and ignored (no mtab on peios)"))
        .arg(boolean(opt::INTERNAL_ONLY).short('i').long("internal-only").help("Do not invoke a umount.<type> helper"))
        .arg(
            // Append so options may be interspersed with the operands
            // (e.g. `umount /a -f /b`), matching util-linux.
            Arg::new(opt::OPERANDS)
                .action(ArgAction::Append)
                .num_args(1)
                .value_name("TARGET|SOURCE")
                .value_parser(clap::value_parser!(std::ffi::OsString))
                .help("Mount point(s) or source(s) to unmount"),
        )
}
