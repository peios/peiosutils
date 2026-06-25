// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Filesystem and partition identity via libblkid.
//!
//! libblkid is opened at runtime (`dlopen` via `libloading`), exactly as
//! `pu_mount` does, so the crate builds without the util-linux dev files; the
//! package declares the `libblkid.so.1` runtime dependency. A single safe probe
//! populates both the superblock tags (`TYPE`/`VERSION`/`LABEL`/`UUID`) and the
//! partition tags (`PTTYPE`/`PTUUID`/`PART_ENTRY_*`).
//!
//! Any failure — no libblkid, an unopenable/empty device, an ambiguous probe —
//! yields `FsInfo::default()`, i.e. blank columns. That is the honest rendering:
//! the data genuinely isn't there, and lsblk does not fabricate it.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;
use std::sync::OnceLock;

use libloading::{Library, Symbol};

/// `BLKID_SUBLKS_LABEL | _UUID | _TYPE | _VERSION` — the superblock fields the
/// filesystem columns need.
const SUBLKS_FLAGS: c_int = (1 << 1) | (1 << 3) | (1 << 5) | (1 << 8);

/// Filesystem + partition identity for one device, from libblkid.
#[derive(Clone, Debug, Default)]
pub struct FsInfo {
    pub fstype: Option<String>,
    pub fsversion: Option<String>,
    pub label: Option<String>,
    pub uuid: Option<String>,
    /// Partition-table type/uuid (a value present on the *whole disk*).
    pub pttype: Option<String>,
    pub ptuuid: Option<String>,
    /// Per-partition entry tags (present on a *partition*).
    pub part_uuid: Option<String>,
    pub part_label: Option<String>,
    pub part_type: Option<String>,
}

/// Probe `devpath` for its filesystem and partition identity. Returns `None`
/// when libblkid is unavailable or the device cannot be probed; callers treat
/// that as `FsInfo::default()`.
pub fn probe(devpath: &Path) -> Option<FsInfo> {
    let lib = library()?;
    let dev = cstr(devpath.to_string_lossy().as_bytes())?;

    // SAFETY: every call uses the documented libblkid signature; `pr` is checked
    // for null and freed before return.
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
            return None;
        }
        enable_super(pr, 1);
        set_flags(pr, SUBLKS_FLAGS);
        enable_parts(pr, 1);

        let info = if safeprobe(pr) == 0 {
            FsInfo {
                fstype: lookup(lib, pr, c"TYPE"),
                fsversion: lookup(lib, pr, c"VERSION"),
                label: lookup(lib, pr, c"LABEL"),
                uuid: lookup(lib, pr, c"UUID"),
                pttype: lookup(lib, pr, c"PTTYPE"),
                ptuuid: lookup(lib, pr, c"PTUUID"),
                part_uuid: lookup(lib, pr, c"PART_ENTRY_UUID"),
                part_label: lookup(lib, pr, c"PART_ENTRY_NAME"),
                part_type: lookup(lib, pr, c"PART_ENTRY_TYPE"),
            }
        } else {
            FsInfo::default()
        };
        free_probe(pr);
        Some(info)
    }
}

/// Look up a single probe value by key, returning `None` when absent.
///
/// # Safety
/// `pr` must be a live probe from `blkid_new_probe_from_filename`.
unsafe fn lookup(lib: &'static Library, pr: *mut c_void, key: &CStr) -> Option<String> {
    // SAFETY: documented signature; out-params are read only on a 0 return.
    unsafe {
        let f: Symbol<
            unsafe extern "C" fn(*mut c_void, *const c_char, *mut *const c_char, *mut usize) -> c_int,
        > = sym(lib, b"blkid_probe_lookup_value\0")?;
        let mut data: *const c_char = std::ptr::null();
        let mut len: usize = 0;
        if f(pr, key.as_ptr(), &raw mut data, &raw mut len) == 0 && !data.is_null() {
            Some(CStr::from_ptr(data).to_string_lossy().into_owned())
        } else {
            None
        }
    }
}

fn library() -> Option<&'static Library> {
    static LIB: OnceLock<Option<Library>> = OnceLock::new();
    // SAFETY: opening a well-known system library by soname.
    LIB.get_or_init(|| unsafe { Library::new("libblkid.so.1").ok() })
        .as_ref()
}

/// # Safety
/// `T` must exactly match the C signature of the symbol named by `name`.
unsafe fn sym<T>(lib: &'static Library, name: &[u8]) -> Option<Symbol<'static, T>> {
    // SAFETY: caller guarantees `T` matches the symbol's real signature.
    unsafe { lib.get(name) }.ok()
}

fn cstr(s: &[u8]) -> Option<CString> {
    CString::new(s).ok()
}
