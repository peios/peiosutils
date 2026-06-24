// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Source resolution and type probing via libblkid (§7).
//!
//! libblkid is opened at runtime (`dlopen` via `libloading`) rather than linked
//! at build time, so the crate builds without the util-linux dev files; the
//! package declares the `libblkid.so.1` runtime dependency. The handful of
//! entry points used are stable libblkid API.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::OnceLock;

use libloading::{Library, Symbol};

use crate::error::{MountError, Result};

/// Recognised source-tag prefixes that require a libblkid lookup.
const TAG_PREFIXES: &[&[u8]] = &[b"UUID=", b"LABEL=", b"PARTUUID=", b"PARTLABEL="];

/// Resolve a source spec to a device path. Tag forms (`UUID=`/`LABEL=`/…) are
/// evaluated via libblkid; a plain path passes through unchanged.
pub fn resolve_source(raw: &[u8]) -> Result<Vec<u8>> {
    if TAG_PREFIXES.iter().any(|p| raw.starts_with(p)) {
        return evaluate_spec(raw);
    }
    Ok(raw.to_vec())
}

/// Probe the filesystem type of `device` (`-t auto` / omitted, §7) with a safe
/// probe (ambiguous/multiple signatures → error). `auto_fstypes`, when set,
/// constrains the acceptable result.
pub fn probe_type(device: &[u8], auto_fstypes: Option<&[u8]>) -> Result<Vec<u8>> {
    let lib = library()?;
    let dev = cstr(device)?;
    // SAFETY: all calls use the documented libblkid signatures; pointers are
    // valid for the call and we free what libblkid allocates.
    unsafe {
        let new_probe: Symbol<unsafe extern "C" fn(*const c_char) -> *mut c_void> =
            sym(lib, b"blkid_new_probe_from_filename\0")?;
        let do_safeprobe: Symbol<unsafe extern "C" fn(*mut c_void) -> c_int> =
            sym(lib, b"blkid_do_safeprobe\0")?;
        let lookup: Symbol<
            unsafe extern "C" fn(*mut c_void, *const c_char, *mut *const c_char, *mut usize) -> c_int,
        > = sym(lib, b"blkid_probe_lookup_value\0")?;
        let free_probe: Symbol<unsafe extern "C" fn(*mut c_void)> = sym(lib, b"blkid_free_probe\0")?;

        let pr = new_probe(dev.as_ptr());
        if pr.is_null() {
            return Err(probe_err(device, "cannot open for probing"));
        }
        let rc = do_safeprobe(pr);
        let result = match rc {
            0 => {
                let key = c"TYPE";
                let mut data: *const c_char = std::ptr::null();
                let mut len: usize = 0;
                if lookup(pr, key.as_ptr(), &raw mut data, &raw mut len) == 0 && !data.is_null() {
                    Ok(CStr::from_ptr(data).to_bytes().to_vec())
                } else {
                    Err(probe_err(device, "no filesystem type found"))
                }
            }
            -2 => Err(probe_err(device, "ambiguous: multiple filesystem signatures")),
            1 => Err(probe_err(device, "no filesystem signature found")),
            _ => Err(probe_err(device, "probe failed")),
        };
        free_probe(pr);

        let fstype = result?;
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
}

/// Look up the LABEL of a device for listing (`-l`), or `None`.
pub fn device_label(device: &[u8]) -> Option<Vec<u8>> {
    let lib = library().ok()?;
    let dev = cstr(device).ok()?;
    let tag = c"LABEL";
    // SAFETY: blkid_get_tag_value(cache=NULL, type, devname) returns a malloc'd
    // string or NULL; we copy and free it.
    unsafe {
        let get_tag: Symbol<
            unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *mut c_char,
        > = sym(lib, b"blkid_get_tag_value\0").ok()?;
        let p = get_tag(std::ptr::null_mut(), tag.as_ptr(), dev.as_ptr());
        if p.is_null() {
            return None;
        }
        let out = CStr::from_ptr(p).to_bytes().to_vec();
        free_c(p.cast());
        Some(out)
    }
}

fn evaluate_spec(spec: &[u8]) -> Result<Vec<u8>> {
    let lib = library()?;
    let c = cstr(spec)?;
    // SAFETY: blkid_evaluate_spec(spec, cache=NULL) returns a malloc'd device
    // path or NULL; we copy and free it.
    unsafe {
        let eval: Symbol<unsafe extern "C" fn(*const c_char, *mut c_void) -> *mut c_char> =
            sym(lib, b"blkid_evaluate_spec\0")?;
        let p = eval(c.as_ptr(), std::ptr::null_mut());
        if p.is_null() {
            return Err(MountError::Mount {
                stage: "blkid",
                source: std::io::Error::other(format!(
                    "cannot find device for {}",
                    String::from_utf8_lossy(spec)
                )),
            });
        }
        let out = CStr::from_ptr(p).to_bytes().to_vec();
        free_c(p.cast());
        Ok(out)
    }
}

fn library() -> Result<&'static Library> {
    static LIB: OnceLock<Option<Library>> = OnceLock::new();
    // SAFETY: opening a well-known system library by soname.
    LIB.get_or_init(|| unsafe { Library::new("libblkid.so.1").ok() })
        .as_ref()
        .ok_or_else(|| {
            MountError::System("libblkid.so.1 is not available (required for tag/auto)".to_string())
        })
}

/// Look up a libblkid symbol.
///
/// # Safety
/// `T` must exactly match the real C signature of the symbol named by `name`;
/// calling through a mismatched type is undefined behaviour.
unsafe fn sym<T>(lib: &'static Library, name: &[u8]) -> Result<Symbol<'static, T>> {
    // SAFETY: caller guarantees `T` matches the symbol's real signature.
    unsafe { lib.get(name) }.map_err(|_| {
        MountError::Bug(format!("libblkid is missing symbol {}", String::from_utf8_lossy(name)))
    })
}

fn free_c(p: *mut c_void) {
    // SAFETY: `p` was allocated by libblkid via the C allocator.
    unsafe { libc::free(p) }
}

fn cstr(s: &[u8]) -> Result<CString> {
    CString::new(s).map_err(|_| MountError::Usage("argument contains a NUL byte".to_string()))
}

fn probe_err(device: &[u8], msg: &str) -> MountError {
    MountError::Mount {
        stage: "probe",
        source: std::io::Error::other(format!("{}: {msg}", String::from_utf8_lossy(device))),
    }
}
