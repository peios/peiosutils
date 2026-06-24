// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! KACS mount-policy step (§8): atomic set-before-attach on the detached
//! `fsmount` fd, via the `peios` crate's `File::mount_set_policy`.
//!
//! The policy is set on the DETACHED mount (before `move_mount`), so on any
//! failure the fd is simply dropped and nothing is ever attached with an
//! unintended policy (§8.3) — no rollback. `kacs_set_mount_policy` resolves the
//! superblock via `fget_raw`, which accepts the O_PATH-style `fsmount` fd
//! (provium-verified, see test-suite mount-newapi).

use std::os::fd::{BorrowedFd, FromRawFd, OwnedFd};

use peios::file::{File, MountPolicy, MountPolicyKind};

use crate::error::{MountError, Result};
use crate::options::PolicyKind;

/// Apply the requested KACS mount policy to a detached `fsmount` fd, *before*
/// `move_mount`. A no-op when no policy is requested.
pub fn apply_set_before_attach(
    detached: BorrowedFd<'_>,
    policy: Option<PolicyKind>,
    sddl: Option<&[u8]>,
) -> Result<()> {
    let Some(kind) = policy else {
        // sddl-without-policy is already rejected during request validation.
        return Ok(());
    };

    let mount_policy = MountPolicy {
        kind: map_kind(kind),
        flags: 0,
        generation: 0,
        template_sd: parse_template(sddl)?,
    };

    // Borrow the fd as an owned File for the call without taking ownership of
    // the caller's fd: dup it, wrap, set policy, and let the dup drop.
    let owned = dup(detached)?;
    let file = File::from(owned);
    file.mount_set_policy(&mount_policy).map_err(translate)
}

fn map_kind(kind: PolicyKind) -> MountPolicyKind {
    match kind {
        PolicyKind::DenyMissing => MountPolicyKind::DENY_MISSING,
        PolicyKind::SynthEphemeral => MountPolicyKind::SYNTHESIZE_EPHEMERAL,
        PolicyKind::SynthPersist => MountPolicyKind::SYNTHESIZE_PERSISTENT,
    }
}

/// §8.4 client-side pre-validation, runnable without an fd (so it applies on
/// every path, including `--fake`): a `--synth-sddl` value must be valid SDDL
/// with an owner. A no-op when no template is given.
pub fn validate_synth_sddl(sddl: Option<&[u8]>) -> Result<()> {
    parse_template(sddl).map(|_| ())
}

/// Parse and validate the `--synth-sddl` template (§8.4): valid SDDL with an
/// owner. Size is enforced by the kernel (surfaced via [`translate`]).
fn parse_template(sddl: Option<&[u8]>) -> Result<Option<peios::security::SecurityDescriptor>> {
    let Some(bytes) = sddl else {
        return Ok(None);
    };
    let text = std::str::from_utf8(bytes)
        .map_err(|_| MountError::Usage("--synth-sddl must be valid SDDL text".to_string()))?;
    let sd = peios::security::sddl::parse(text)
        .map_err(|e| MountError::Usage(format!("invalid --synth-sddl: {e}")))?;
    let has_owner = sd
        .view()
        .ok()
        .and_then(|v| v.owner().map(|_| ()))
        .is_some();
    if !has_owner {
        return Err(MountError::Usage(
            "--synth-sddl must include an owner (e.g. O:SY)".to_string(),
        ));
    }
    Ok(Some(sd))
}

/// §8.5 error translation.
fn translate(e: peios::Error) -> MountError {
    let io = || std::io::Error::from_raw_os_error(e.raw_os_error().unwrap_or(0));
    match e.raw_os_error() {
        Some(libc::EOPNOTSUPP) => MountError::Mount {
            stage: "set_mount_policy",
            source: std::io::Error::other(
                "filesystem type does not support mount policies (it is unmanaged)",
            ),
        },
        // EPERM folds to exit 1 with a SeTcbPrivilege hint.
        Some(libc::EPERM) => MountError::Permission {
            stage: "set_mount_policy (requires SeTcbPrivilege)",
            source: io(),
        },
        _ => MountError::from_syscall("set_mount_policy", io()),
    }
}

/// Duplicate a borrowed fd into an owned one (so a `peios::File` can take it
/// without consuming the caller's fd).
fn dup(fd: BorrowedFd<'_>) -> Result<OwnedFd> {
    use std::os::fd::AsRawFd;
    // SAFETY: dup of a valid fd; the result is a fresh owned descriptor.
    let raw = unsafe { libc::dup(fd.as_raw_fd()) };
    if raw < 0 {
        return Err(MountError::from_syscall("dup", std::io::Error::last_os_error()));
    }
    // SAFETY: `raw` is a fresh, owned fd from dup.
    Ok(unsafe { OwnedFd::from_raw_fd(raw) })
}
