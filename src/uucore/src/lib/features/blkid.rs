// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Filesystem and partition identity via libblkid.
//!
//! libblkid is opened at runtime (`dlopen` via `libloading`) rather than linked
//! at build time, so consuming crates build without the util-linux dev files;
//! the package declares the `libblkid.so.1` runtime dependency. The handful of
//! entry points used are stable libblkid API.
//!
//! This lives in uucore because three tools need it and each had been growing
//! its own copy: `mount` resolves `UUID=`/`LABEL=` source specs and probes
//! `-t auto`, `lsblk` fills its filesystem and partition columns, and `part`
//! asks what kind of partition table a disk carries before it will touch it.
//!
//! # What belongs here, and what does not
//!
//! Identifying a foreign format is a *data* problem — a catalogue of other
//! people's magic numbers that grows with every util-linux release, and one
//! that is silently wrong rather than loudly wrong when it is out of date. That
//! is exactly the kind of thing to reuse rather than reimplement.
//!
//! Deciding what to *write* is not. `part` builds GPTs itself and only asks
//! this module what is already on the disk. In particular libblkid reports
//! `PTTYPE=gpt` on the strength of a protective MBR alone, so "is this GPT
//! healthy?" is a question its caller answers, not this module.
//!
//! # Error model
//!
//! [`probe`] reports precisely why it failed, because `mount` must distinguish
//! "several filesystem signatures, refusing to guess" from "no signature at
//! all" — they mean different things to a user. Callers that would rather have
//! blank output than a diagnostic use [`probe_lossy`], which is `lsblk`'s
//! behaviour: the data genuinely is not there, and inventing it would be worse.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::fmt;
use std::path::Path;
use std::sync::OnceLock;

use libloading::{Library, Symbol};

/// `BLKID_SUBLKS_LABEL | _UUID | _TYPE | _VERSION` — the superblock fields
/// every current caller needs.
const SUBLKS_FLAGS: c_int = (1 << 1) | (1 << 3) | (1 << 5) | (1 << 8);

/// Source-tag prefixes that require a libblkid lookup rather than being a path.
pub const TAG_PREFIXES: &[&[u8]] = &[b"UUID=", b"LABEL=", b"PARTUUID=", b"PARTLABEL="];

/// Filesystem and partition identity for one device.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlkidInfo {
    pub fstype: Option<String>,
    pub fsversion: Option<String>,
    pub label: Option<String>,
    pub uuid: Option<String>,
    /// Partition-table type/uuid — present on the *whole disk*.
    pub pttype: Option<String>,
    pub ptuuid: Option<String>,
    /// Per-partition entry tags — present on a *partition*.
    pub part_uuid: Option<String>,
    pub part_label: Option<String>,
    pub part_type: Option<String>,
}

impl BlkidInfo {
    /// True when nothing at all was recognised — no filesystem and no partition
    /// table. For `part` this is the "blank disk, safe to create" case, and it
    /// is deliberately distinct from a probe *failure*.
    pub fn is_empty(&self) -> bool {
        self.fstype.is_none() && self.pttype.is_none()
    }
}

/// Why a probe did not produce an answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlkidError {
    /// libblkid is not installed. Callers decide whether that is fatal.
    Unavailable,
    /// The library is present but lacks an expected symbol — a packaging fault.
    MissingSymbol(String),
    /// The device could not be opened for probing.
    OpenFailed,
    /// Several filesystem signatures are present; libblkid refuses to guess and
    /// so do we. Overwriting one of them would be the wrong kind of decision to
    /// make on a user's behalf.
    Ambiguous,
    /// Probed cleanly and found nothing.
    NoSignature,
    /// A `UUID=`/`LABEL=`/… spec matched no device.
    ///
    /// Deliberately distinct from `NoSignature`: the question asked was "which
    /// device is this?", not "what is on it?", and a caller resolving a source
    /// spec needs to say "cannot find device for UUID=…" rather than "no
    /// filesystem signature found". Conflating them was a latent bug here —
    /// unnoticed only because the sole caller at the time never resolved specs.
    SpecUnresolved,
    /// libblkid failed for a reason it did not distinguish.
    ProbeFailed,
    /// An argument contained an interior NUL and cannot cross the FFI boundary.
    Nul,
}

impl fmt::Display for BlkidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => write!(f, "libblkid.so.1 is not available"),
            Self::MissingSymbol(s) => write!(f, "libblkid is missing symbol {s}"),
            Self::OpenFailed => write!(f, "cannot open for probing"),
            Self::Ambiguous => write!(f, "ambiguous: multiple filesystem signatures"),
            Self::NoSignature => write!(f, "no filesystem signature found"),
            Self::SpecUnresolved => write!(f, "cannot find a device matching the spec"),
            Self::ProbeFailed => write!(f, "probe failed"),
            Self::Nul => write!(f, "argument contains a NUL byte"),
        }
    }
}

impl std::error::Error for BlkidError {}

/// Which libblkid probe chains to enable.
///
/// This is not a tuning knob — it changes what counts as an error.
/// `blkid_do_safeprobe` propagates `-2` (ambiguous) from **any** enabled chain,
/// so enabling partitions means a disk carrying conflicting partition tables —
/// a hybrid MBR/GPT, say — becomes ambiguous even when its filesystem
/// signature is perfectly clear. A caller that only wants the filesystem type
/// must not be failed by a partition-table disagreement it never asked about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chains {
    /// libblkid's own defaults: superblocks only, default flags. What
    /// `mount -t auto` has always used, and the safest answer to "what
    /// filesystem is this?".
    Default,
    /// Superblocks with LABEL/UUID/TYPE/VERSION, plus the partition chain.
    /// What `lsblk` and `part` want, since they report `PTTYPE` too.
    WithPartitions,
}

/// Probe a device for its filesystem and partition identity, enabling both
/// chains. See [`probe_with`] when the partition chain must stay out of it.
pub fn probe(device: &[u8]) -> Result<BlkidInfo, BlkidError> {
    probe_with(device, Chains::WithPartitions)
}

/// Probe a device with an explicit chain selection.
pub fn probe_with(device: &[u8], chains: Chains) -> Result<BlkidInfo, BlkidError> {
    let lib = library()?;
    let dev = cstr(device)?;

    // SAFETY: every call below uses the documented libblkid signature. `pr` is
    // null-checked before use and freed on every path out.
    unsafe {
        let new_probe: Symbol<unsafe extern "C" fn(*const c_char) -> *mut c_void> =
            sym(lib, b"blkid_new_probe_from_filename\0")?;
        let enable_super: Symbol<unsafe extern "C" fn(*mut c_void, c_int) -> c_int> =
            sym(lib, b"blkid_probe_enable_superblocks\0")?;
        let set_flags: Symbol<unsafe extern "C" fn(*mut c_void, c_int) -> c_int> =
            sym(lib, b"blkid_probe_set_superblocks_flags\0")?;
        let enable_parts: Symbol<unsafe extern "C" fn(*mut c_void, c_int) -> c_int> =
            sym(lib, b"blkid_probe_enable_partitions\0")?;
        let safeprobe: Symbol<unsafe extern "C" fn(*mut c_void) -> c_int> =
            sym(lib, b"blkid_do_safeprobe\0")?;
        let free_probe: Symbol<unsafe extern "C" fn(*mut c_void)> =
            sym(lib, b"blkid_free_probe\0")?;

        let pr = new_probe(dev.as_ptr());
        if pr.is_null() {
            return Err(BlkidError::OpenFailed);
        }
        // Default leaves the chains exactly as libblkid configures them —
        // deliberately not "superblocks on, partitions off", because touching
        // the chain configuration at all is what this variant exists to avoid.
        if chains == Chains::WithPartitions {
            enable_super(pr, 1);
            set_flags(pr, SUBLKS_FLAGS);
            enable_parts(pr, 1);
        }

        // blkid_do_safeprobe: 0 success, 1 nothing found, -2 ambiguous.
        let rc = safeprobe(pr);
        let out = match rc {
            0 => Ok(BlkidInfo {
                fstype: lookup(lib, pr, c"TYPE"),
                fsversion: lookup(lib, pr, c"VERSION"),
                label: lookup(lib, pr, c"LABEL"),
                uuid: lookup(lib, pr, c"UUID"),
                pttype: lookup(lib, pr, c"PTTYPE"),
                ptuuid: lookup(lib, pr, c"PTUUID"),
                part_uuid: lookup(lib, pr, c"PART_ENTRY_UUID"),
                part_label: lookup(lib, pr, c"PART_ENTRY_NAME"),
                part_type: lookup(lib, pr, c"PART_ENTRY_TYPE"),
            }),
            1 => Err(BlkidError::NoSignature),
            -2 => Err(BlkidError::Ambiguous),
            _ => Err(BlkidError::ProbeFailed),
        };
        free_probe(pr);
        out
    }
}

/// [`probe`] for a `Path`.
pub fn probe_path(device: &Path) -> Result<BlkidInfo, BlkidError> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        probe(device.as_os_str().as_bytes())
    }
    #[cfg(not(unix))]
    {
        probe(device.to_string_lossy().as_bytes())
    }
}

/// Best-effort probe: any failure, including a device with nothing on it,
/// becomes `None`.
///
/// This is the right shape for a *renderer* — `lsblk` leaves the column blank
/// rather than fabricating a value — and the wrong shape for anything that acts
/// on the answer, which wants to know why.
pub fn probe_lossy(device: &Path) -> Option<BlkidInfo> {
    match probe_path(device) {
        Ok(info) => Some(info),
        // A device that probes cleanly and holds nothing is still a successful
        // observation; report it as empty rather than as absent.
        Err(BlkidError::NoSignature) => Some(BlkidInfo::default()),
        Err(_) => None,
    }
}

/// Resolve a `UUID=`/`LABEL=`/`PARTUUID=`/`PARTLABEL=` spec to a device path.
/// A spec carrying none of those prefixes is returned unchanged.
pub fn resolve_spec(raw: &[u8]) -> Result<Vec<u8>, BlkidError> {
    if !TAG_PREFIXES.iter().any(|p| raw.starts_with(p)) {
        return Ok(raw.to_vec());
    }
    let lib = library()?;
    let c = cstr(raw)?;
    // SAFETY: blkid_evaluate_spec(spec, cache=NULL) returns a malloc'd device
    // path or NULL; we copy it out and free the original.
    unsafe {
        let eval: Symbol<unsafe extern "C" fn(*const c_char, *mut c_void) -> *mut c_char> =
            sym(lib, b"blkid_evaluate_spec\0")?;
        let p = eval(c.as_ptr(), std::ptr::null_mut());
        if p.is_null() {
            return Err(BlkidError::SpecUnresolved);
        }
        let out = CStr::from_ptr(p).to_bytes().to_vec();
        libc::free(p.cast());
        Ok(out)
    }
}

/// Look up a single tag (`LABEL`, `UUID`, …) for a device, or `None`.
pub fn tag_value(tag: &CStr, device: &[u8]) -> Option<Vec<u8>> {
    let lib = library().ok()?;
    let dev = cstr(device).ok()?;
    // SAFETY: blkid_get_tag_value(cache=NULL, type, devname) returns a malloc'd
    // string or NULL; we copy it out and free the original.
    unsafe {
        let get_tag: Symbol<
            unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *mut c_char,
        > = sym(lib, b"blkid_get_tag_value\0").ok()?;
        let p = get_tag(std::ptr::null_mut(), tag.as_ptr(), dev.as_ptr());
        if p.is_null() {
            return None;
        }
        let out = CStr::from_ptr(p).to_bytes().to_vec();
        libc::free(p.cast());
        Some(out)
    }
}

/// Whether libblkid could be opened at all.
pub fn available() -> bool {
    library().is_ok()
}

/// Look up a single probe value by key, returning `None` when absent.
///
/// # Safety
/// `pr` must be a live probe from `blkid_new_probe_from_filename`.
unsafe fn lookup(lib: &'static Library, pr: *mut c_void, key: &CStr) -> Option<String> {
    // SAFETY: documented signature; the out-params are read only on a 0 return.
    unsafe {
        let f: Symbol<
            unsafe extern "C" fn(*mut c_void, *const c_char, *mut *const c_char, *mut usize) -> c_int,
        > = lib.get(b"blkid_probe_lookup_value\0").ok()?;
        let mut data: *const c_char = std::ptr::null();
        let mut len: usize = 0;
        if f(pr, key.as_ptr(), &raw mut data, &raw mut len) == 0 && !data.is_null() {
            Some(CStr::from_ptr(data).to_string_lossy().into_owned())
        } else {
            None
        }
    }
}

fn library() -> Result<&'static Library, BlkidError> {
    static LIB: OnceLock<Option<Library>> = OnceLock::new();
    // SAFETY: opening a well-known system library by soname.
    LIB.get_or_init(|| unsafe { Library::new("libblkid.so.1").ok() })
        .as_ref()
        .ok_or(BlkidError::Unavailable)
}

/// # Safety
/// `T` must exactly match the real C signature of the symbol named by `name`;
/// calling through a mismatched type is undefined behaviour.
unsafe fn sym<T>(lib: &'static Library, name: &[u8]) -> Result<Symbol<'static, T>, BlkidError> {
    // SAFETY: the caller guarantees `T` matches the symbol's real signature.
    unsafe { lib.get(name) }.map_err(|_| {
        BlkidError::MissingSymbol(String::from_utf8_lossy(name).trim_end_matches('\0').to_string())
    })
}

fn cstr(s: &[u8]) -> Result<CString, BlkidError> {
    CString::new(s).map_err(|_| BlkidError::Nul)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_means_no_filesystem_and_no_table() {
        assert!(BlkidInfo::default().is_empty());
        assert!(!BlkidInfo {
            pttype: Some("gpt".into()),
            ..Default::default()
        }
        .is_empty());
        assert!(!BlkidInfo {
            fstype: Some("ext4".into()),
            ..Default::default()
        }
        .is_empty());
    }

    #[test]
    fn tag_prefixes_are_recognised() {
        for spec in [
            b"UUID=1234".as_slice(),
            b"LABEL=root",
            b"PARTUUID=abcd",
            b"PARTLABEL=esp",
        ] {
            assert!(TAG_PREFIXES.iter().any(|p| spec.starts_with(p)), "{spec:?}");
        }
        assert!(!TAG_PREFIXES.iter().any(|p| b"/dev/vda1".starts_with(p)));
    }

    /// A plain path must pass through `resolve_spec` untouched even when
    /// libblkid is absent — otherwise every device argument would depend on a
    /// library that has nothing to say about it.
    #[test]
    fn plain_paths_resolve_without_libblkid() {
        assert_eq!(resolve_spec(b"/dev/vda1").unwrap(), b"/dev/vda1".to_vec());
    }

    #[test]
    fn errors_render_usefully() {
        assert_eq!(
            BlkidError::Ambiguous.to_string(),
            "ambiguous: multiple filesystem signatures"
        );
        assert_eq!(
            BlkidError::MissingSymbol("blkid_do_safeprobe".into()).to_string(),
            "libblkid is missing symbol blkid_do_safeprobe"
        );
    }
}
