// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! `OWNER` / `MODE` (the `-m`/`--perms` columns), read from the device node's
//! Security Descriptor.
//!
//! These mirror peios `ls -l` exactly (see `uu/ls/src/display.rs` and the
//! `peiosutils-sd-perms-mirror-ls` design note): there are no POSIX permission
//! bits and no `GROUP`, because access is governed by the SD, not a mode. A
//! device contributes:
//! * `OWNER` — the owner SID (`S-1-…`), or `?` when the SD can't be read;
//! * `MODE` — the three-character `[type][x][+]`: device-type char (`b` block,
//!   `c` char), `x` if executable-marked (never, for a block device), and `+`
//!   if the DACL is inheritance-protected.
//!
//! The SD read goes through [`uucore::sd_control::read_sd_display`], the same
//! seam `ls` uses; a kernel without the KACS SD syscalls (or an SD-less `/dev`)
//! yields `?`/`-`, which is the honest rendering.

use std::os::unix::fs::FileTypeExt;
use std::path::Path;

/// Owner SID + mode triad for one device node.
pub struct Perms {
    pub owner: Option<String>,
    pub mode: String,
}

/// Read the device node's SD and render the `ls -l`-style owner + mode.
pub fn read(devpath: &Path) -> Perms {
    // TODO(substrate): OWNER/MODE only carry real data on peios, where the KACS
    // SD syscalls exist and /dev is a managed (SD-bearing) filesystem. On a
    // stock-Linux host (no KACS) OWNER degrades to `?` and MODE to `b--` — by
    // design, the honest-degrade. Nothing to add here until running on peios.
    let sd = uucore::sd_control::read_sd_display(devpath, true);
    let prot = if sd.protected_dacl { '+' } else { '-' };
    // A block device is never executable-marked; the middle slot stays `-`,
    // matching what `ls -l` shows for a device node.
    let mode = format!("{}{}{prot}", type_char(devpath), '-');
    Perms { owner: sd.owner, mode }
}

/// `b` for a block device, `c` for a char device. We already know from sysfs
/// that this is a block device, so a failed stat falls back to `b` rather than
/// dropping the type information.
fn type_char(devpath: &Path) -> char {
    match std::fs::metadata(devpath) {
        Ok(md) if md.file_type().is_char_device() => 'c',
        _ => 'b',
    }
}
