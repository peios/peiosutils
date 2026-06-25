// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! The column registry: identity, headers, alignment, and value extraction.
//!
//! Each [`Column`] knows its `-o`/`-P`/JSON key, its table header, whether it is
//! right-aligned, and how to read its value from a [`Device`]. Values are
//! `Option<String>`: `None` is an empty cell (table), an empty string (pairs),
//! or `null` (JSON).

use crate::config::Config;
use crate::device::Device;
use crate::error::{LsblkError, Result};
use crate::perms;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Name,
    KName,
    Path,
    MajMin,
    FsType,
    FsVer,
    Label,
    Uuid,
    PtUuid,
    PtType,
    PartType,
    PartLabel,
    PartUuid,
    Mountpoint,
    Mountpoints,
    Size,
    Ro,
    Rm,
    Type,
    Owner,
    Mode,
    Model,
    Vendor,
    Rev,
    Serial,
    Tran,
    Hctl,
    State,
    Rota,
    Sched,
    PkName,
    IdLink,
    PhySec,
    LogSec,
    MinIo,
    OptIo,
    Alignment,
    Hotplug,
}

use Column::*;

/// All columns, in the order `-O`/`--output-all` emits them.
pub const ALL: &[Column] = &[
    Name, KName, Path, MajMin, FsType, FsVer, Label, Uuid, PtUuid, PtType, PartType, PartLabel,
    PartUuid, Mountpoint, Mountpoints, Size, Ro, Rm, Hotplug, Type, Owner, Mode, Model, Vendor, Rev,
    Serial, Tran, Hctl, Alignment, MinIo, OptIo, PhySec, LogSec, State, Rota, Sched, PkName, IdLink,
];

/// Default columns (`lsblk` with no `-o`).
pub const DEFAULT: &[Column] = &[Name, MajMin, Rm, Size, Ro, Type, Mountpoints];

/// `-f`/`--fs`.
pub const FS: &[Column] = &[Name, FsType, FsVer, Label, Uuid, Mountpoints];

/// `-m`/`--perms`.
pub const PERMS: &[Column] = &[Name, Size, Owner, Mode];

// Columns upstream has that this port deliberately omits:
//
// TODO(substrate): blocked on peios substrate that doesn't exist yet —
//   * WWN — needs udev's `ID_WWN` (sysfs `wwid` exists only on some devices);
//     blocked on peios-udev, same as ID-LINK's fast-path (see config::build_idlink).
//   * ZONED / ZONE-SZ / ZONE-WGRAN / ZONE-APP / ZONE-NR / ZONE-OMAX / ZONE-AMAX —
//     need the zoned block-device (ZBD) model.
//   * DAX — needs the DAX / pmem stack.
//
// TODO: NOT substrate-blocked, just unimplemented — add when needed:
//   * FSAVAIL / FSSIZE / FSUSED / FSUSE% / FSROOTS — `statvfs(2)` of the mount
//     point (only meaningful for mounted filesystems).
//   * DISC-ALN / DISC-GRAN / DISC-MAX / DISC-ZERO — sysfs `queue/discard_*`.
//   * RA / RQ-SIZE / SUBSYSTEMS / DAX — further sysfs `queue/*` and bus reads.

impl Column {
    /// The `-o`/`-P`/JSON key (e.g. `MAJ:MIN`, `FSTYPE`).
    pub fn id(self) -> &'static str {
        match self {
            Name => "NAME",
            KName => "KNAME",
            Path => "PATH",
            MajMin => "MAJ:MIN",
            FsType => "FSTYPE",
            FsVer => "FSVER",
            Label => "LABEL",
            Uuid => "UUID",
            PtUuid => "PTUUID",
            PtType => "PTTYPE",
            PartType => "PARTTYPE",
            PartLabel => "PARTLABEL",
            PartUuid => "PARTUUID",
            Mountpoint => "MOUNTPOINT",
            Mountpoints => "MOUNTPOINTS",
            Size => "SIZE",
            Ro => "RO",
            Rm => "RM",
            Type => "TYPE",
            Owner => "OWNER",
            Mode => "MODE",
            Model => "MODEL",
            Vendor => "VENDOR",
            Rev => "REV",
            Serial => "SERIAL",
            Tran => "TRAN",
            Hctl => "HCTL",
            State => "STATE",
            Rota => "ROTA",
            Sched => "SCHED",
            PkName => "PKNAME",
            IdLink => "ID-LINK",
            PhySec => "PHY-SEC",
            LogSec => "LOG-SEC",
            MinIo => "MIN-IO",
            OptIo => "OPT-IO",
            Alignment => "ALIGNMENT",
            Hotplug => "HOTPLUG",
        }
    }

    /// Right-aligned in table output (numbers).
    pub fn right_aligned(self) -> bool {
        matches!(
            self,
            MajMin | Size | Ro | Rm | Rota | Hotplug | Alignment | MinIo | OptIo | PhySec | LogSec
        )
    }

    /// A 0/1 flag column. Emitted as a bare JSON boolean (upstream `lsblk -J`
    /// renders these as `true`/`false`, not strings).
    pub fn is_boolean(self) -> bool {
        matches!(self, Ro | Rm | Rota | Hotplug)
    }

    /// Free-text columns whose values get hex-escaped in raw (`-r`) output, the
    /// same set upstream escapes — anything that could carry a space or control
    /// char and break a field-split parser.
    pub fn raw_escaped(self) -> bool {
        matches!(
            self,
            Name | KName | Path | Label | PartLabel | Uuid | PartUuid | Mountpoint | Mountpoints
        )
    }

    /// Resolve an `-o` token (case-insensitive) to a column.
    pub fn parse(token: &str) -> Result<Self> {
        let upper = token.trim().to_ascii_uppercase();
        ALL.iter()
            .copied()
            .find(|c| c.id() == upper)
            .ok_or_else(|| LsblkError::Usage(format!("unknown column: {token}")))
    }

    /// The value for `dev` (None ⇒ empty/null).
    pub fn value(self, dev: &Device, config: &Config) -> Option<String> {
        match self {
            Name => Some(if config.paths {
                dev.devpath.to_string_lossy().into_owned()
            } else {
                dev.name.clone()
            }),
            KName => Some(dev.name.clone()),
            Path => Some(dev.devpath.to_string_lossy().into_owned()),
            MajMin => Some(format!("{}:{}", dev.maj, dev.min)),
            FsType => dev.fs.fstype.clone(),
            FsVer => dev.fs.fsversion.clone(),
            Label => dev.fs.label.clone(),
            Uuid => dev.fs.uuid.clone(),
            PtUuid => dev.fs.ptuuid.clone(),
            PtType => dev.fs.pttype.clone(),
            PartType => dev.fs.part_type.clone(),
            PartLabel => dev.fs.part_label.clone(),
            PartUuid => dev.fs.part_uuid.clone(),
            Mountpoint => dev.mountpoints.first().cloned(),
            Mountpoints => {
                if dev.mountpoints.is_empty() {
                    None
                } else {
                    Some(dev.mountpoints.join("\n"))
                }
            }
            Size => Some(if config.bytes {
                dev.size_bytes.to_string()
            } else {
                human_size(dev.size_bytes)
            }),
            Ro => Some(bit(dev.read_only)),
            Rm => Some(bit(dev.removable)),
            Type => Some(dev.dtype.as_str().to_string()),
            Owner => perms::read(&dev.devpath).owner,
            Mode => Some(perms::read(&dev.devpath).mode),
            Model => dev.model.clone(),
            Vendor => dev.vendor.clone(),
            Rev => dev.rev.clone(),
            Serial => dev.serial.clone(),
            Tran => dev.transport.clone(),
            Hctl => dev.hctl.clone(),
            State => dev.state.clone(),
            Rota => Some(bit(dev.rotational.unwrap_or(false))),
            Sched => dev.scheduler.clone(),
            PkName => dev.parent_kname.clone(),
            IdLink => config.idlink.get(&(dev.maj, dev.min)).cloned(),
            PhySec => dev.phy_sec.map(|v| v.to_string()),
            LogSec => dev.log_sec.map(|v| v.to_string()),
            MinIo => dev.min_io.map(|v| v.to_string()),
            OptIo => dev.opt_io.map(|v| v.to_string()),
            Alignment => dev.alignment.map(|v| v.to_string()),
            Hotplug => Some(bit(dev.hotplug)),
        }
    }
}

/// Parse a comma-separated `-o` list into columns (rejecting unknowns).
pub fn parse_list(spec: &str) -> Result<Vec<Column>> {
    spec.split(',')
        .filter(|t| !t.trim().is_empty())
        .map(Column::parse)
        .collect()
}

fn bit(b: bool) -> String {
    if b { "1" } else { "0" }.to_string()
}

/// Human-readable size with binary (1024) units, one decimal when fractional —
/// matching lsblk's default SIZE rendering closely enough for scripts that
/// switch to `-b` when they need exact bytes.
pub fn human_size(n: u64) -> String {
    const UNITS: [&str; 8] = ["B", "K", "M", "G", "T", "P", "E", "Z"];
    let mut val = n as f64;
    let mut i = 0;
    while val >= 1024.0 && i < UNITS.len() - 1 {
        val /= 1024.0;
        i += 1;
    }
    if i == 0 {
        return format!("{n}B");
    }
    let rounded = (val * 10.0).round() / 10.0;
    if rounded.fract().abs() < 1e-9 {
        format!("{rounded:.0}{}", UNITS[i])
    } else {
        format!("{rounded:.1}{}", UNITS[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_columns_case_insensitively() {
        assert_eq!(Column::parse("name").unwrap(), Name);
        assert_eq!(Column::parse("MAJ:MIN").unwrap(), MajMin);
        assert_eq!(Column::parse("fstype").unwrap(), FsType);
    }

    #[test]
    fn rejects_unknown_column() {
        assert!(Column::parse("nonsense").is_err());
    }

    #[test]
    fn human_size_units() {
        assert_eq!(human_size(0), "0B");
        assert_eq!(human_size(512), "512B");
        assert_eq!(human_size(1024), "1K");
        assert_eq!(human_size(1536), "1.5K");
        assert_eq!(human_size(250_000_000_000), "232.8G");
    }
}
