// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Source resolution and type probing via libblkid (§7).
//!
//! The libblkid FFI itself lives in [`uucore::blkid`], shared with `lsblk` and
//! `part`; this module is the thin layer that maps its results onto
//! [`MountError`]. It used to carry its own `dlopen` shim — the third of three
//! near-identical copies — and keeping the mapping here rather than pushing
//! mount's vocabulary into uucore is deliberate: the messages and exit codes
//! below are mount's, and §7's contract is stated in exit codes (32 for an
//! unresolvable tag, 32 for an ambiguous probe), which come from the
//! `MountError` *kind* chosen here.
//!
//! Two details are load-bearing and easy to lose in a refactor:
//!
//! * **Chain selection.** Probing uses [`Chains::Default`], libblkid's own
//!   configuration, because `blkid_do_safeprobe` propagates "ambiguous" from
//!   any enabled chain. Enabling the partition chain would make a hybrid
//!   MBR/GPT disk fail `-t auto` over a disagreement about partition tables
//!   that has nothing to do with the filesystem being mounted.
//! * **"No TYPE" is not "no signature".** A probe can succeed and still yield
//!   no `TYPE`, which §7 treats as a failure to determine the type — distinct
//!   from finding no signature at all. uucore reports that as a `None` field
//!   rather than an error, so it is re-raised here.

use uucore::blkid::{self, BlkidError, Chains};

use crate::error::{MountError, Result};

/// Resolve a source spec to a device path. Tag forms (`UUID=`/`LABEL=`/…) are
/// evaluated via libblkid; a plain path passes through unchanged.
pub fn resolve_source(raw: &[u8]) -> Result<Vec<u8>> {
    blkid::resolve_spec(raw).map_err(|e| map_resolve_error(raw, e))
}

/// Probe the filesystem type of `device` (`-t auto` / omitted, §7) with a safe
/// probe (ambiguous/multiple signatures → error). `auto_fstypes`, when set,
/// constrains the acceptable result.
pub fn probe_type(device: &[u8], auto_fstypes: Option<&[u8]>) -> Result<Vec<u8>> {
    let info = blkid::probe_with(device, Chains::Default)
        .map_err(|e| map_probe_error(device, e))?;

    // Probed cleanly, but libblkid offered no TYPE. Distinct from the
    // no-signature case above, and the same message the hand-rolled shim gave.
    let fstype = info
        .fstype
        .ok_or_else(|| probe_err(device, "no filesystem type found"))?
        .into_bytes();

    if let Some(list) = auto_fstypes {
        if !list.split(|&b| b == b',').any(|t| t == fstype.as_slice()) {
            return Err(MountError::Mount {
                stage: "probe",
                source: std::io::Error::other(format!(
                    "probed type '{}' is not in X-mount.auto-fstypes",
                    String::from_utf8_lossy(&fstype)
                )),
            });
        }
    }
    Ok(fstype)
}

/// Look up the LABEL of a device for listing (`-l`), or `None`.
pub fn device_label(device: &[u8]) -> Option<Vec<u8>> {
    blkid::tag_value(c"LABEL", device)
}

/// Map a probe failure onto mount's vocabulary.
///
/// Extracted so it can be tested directly: every arm here is a message and an
/// exit code that §7 or a user depends on, and none of them is reachable from
/// a unit test through libblkid itself.
fn map_probe_error(device: &[u8], e: BlkidError) -> MountError {
    match e {
        BlkidError::OpenFailed => probe_err(device, "cannot open for probing"),
        BlkidError::Ambiguous => probe_err(device, "ambiguous: multiple filesystem signatures"),
        BlkidError::NoSignature => probe_err(device, "no filesystem signature found"),
        BlkidError::ProbeFailed => probe_err(device, "probe failed"),
        other => library_err(other, "required for tag/auto"),
    }
}

/// Map a source-resolution failure onto mount's vocabulary.
fn map_resolve_error(raw: &[u8], e: BlkidError) -> MountError {
    match e {
        // §7: a tag matching no device is exit 32, and the message names the
        // spec the user typed rather than the library's generic wording.
        BlkidError::SpecUnresolved => MountError::Mount {
            stage: "blkid",
            source: std::io::Error::other(format!(
                "cannot find device for {}",
                String::from_utf8_lossy(raw)
            )),
        },
        other => library_err(other, "required for tag/auto"),
    }
}

/// Map the errors that are about the library rather than about the device.
/// These keep their own exit codes: a missing library is a system fault (2),
/// a missing symbol is a packaging bug (4), a NUL is bad input (1).
fn library_err(e: BlkidError, why: &str) -> MountError {
    match e {
        BlkidError::Unavailable => {
            MountError::System(format!("libblkid.so.1 is not available ({why})"))
        }
        BlkidError::MissingSymbol(s) => MountError::Bug(format!("libblkid is missing symbol {s}")),
        BlkidError::Nul => MountError::Usage("argument contains a NUL byte".to_string()),
        // Anything else reaching here is a device-level failure a caller did
        // not map; report it as one rather than silently reclassifying it.
        other => MountError::Mount {
            stage: "blkid",
            source: std::io::Error::other(other.to_string()),
        },
    }
}

fn probe_err(device: &[u8], msg: &str) -> MountError {
    MountError::Mount {
        stage: "probe",
        source: std::io::Error::other(format!("{}: {msg}", String::from_utf8_lossy(device))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every message and exit code the hand-rolled shim produced, asserted
    /// against the mapping that replaced it. These are the contract: §7 states
    /// its behaviour in exit codes, and the wording is what a user reads.
    #[test]
    fn probe_failures_keep_their_wording_and_exit_32() {
        for (err, want) in [
            (BlkidError::OpenFailed, "cannot open for probing"),
            (
                BlkidError::Ambiguous,
                "ambiguous: multiple filesystem signatures",
            ),
            (BlkidError::NoSignature, "no filesystem signature found"),
            (BlkidError::ProbeFailed, "probe failed"),
        ] {
            let e = map_probe_error(b"/dev/vdb", err.clone());
            let msg = e.to_string();
            assert!(msg.contains(want), "{err:?} -> {msg}");
            assert!(msg.contains("/dev/vdb"), "must name the device: {msg}");
            assert_eq!(e.exit_code(), 32, "{err:?}");
        }
    }

    /// The distinction uucore did not originally draw: an unresolvable tag is
    /// not "no filesystem signature", and the message names what was typed.
    #[test]
    fn an_unresolvable_spec_names_the_spec_and_exits_32() {
        let e = map_resolve_error(b"UUID=1234-abcd", BlkidError::SpecUnresolved);
        let msg = e.to_string();
        assert!(msg.contains("cannot find device for UUID=1234-abcd"), "{msg}");
        assert_eq!(e.exit_code(), 32);
    }

    /// Library-level faults are not device failures and must not be exit 32:
    /// a missing library is a system fault, a missing symbol is a packaging
    /// bug, and a NUL in an argument is bad input.
    #[test]
    fn library_faults_keep_their_own_exit_codes() {
        let e = library_err(BlkidError::Unavailable, "required for tag/auto");
        assert!(e.to_string().contains("libblkid.so.1 is not available"));
        assert!(e.to_string().contains("required for tag/auto"));
        assert_eq!(e.exit_code(), 2);

        let e = library_err(BlkidError::MissingSymbol("blkid_do_safeprobe".into()), "x");
        assert!(e.to_string().contains("missing symbol blkid_do_safeprobe"));
        assert_eq!(e.exit_code(), 4);

        let e = library_err(BlkidError::Nul, "x");
        assert!(e.to_string().contains("NUL byte"));
        assert_eq!(e.exit_code(), 1);
    }

    /// Both entry points route library faults the same way, so a missing
    /// libblkid reports identically whether it was hit resolving or probing.
    #[test]
    fn both_entry_points_agree_on_library_faults() {
        assert_eq!(
            map_probe_error(b"/dev/vdb", BlkidError::Unavailable).exit_code(),
            map_resolve_error(b"UUID=x", BlkidError::Unavailable).exit_code()
        );
    }

    /// A plain path must not consult libblkid at all — it resolves to itself
    /// even where the library is absent.
    #[test]
    fn a_plain_path_passes_through() {
        assert_eq!(resolve_source(b"/dev/vda1").unwrap(), b"/dev/vda1".to_vec());
    }
}
