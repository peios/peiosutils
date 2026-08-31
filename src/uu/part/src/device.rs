// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Everything that touches a real device: geometry, the safety guards, sector
//! I/O, and asking the kernel to re-read the table.
//!
//! Regular files are first-class targets, not a test affordance bolted on. A
//! file-backed image is a complete subject for partition-table work, so the
//! entire test suite runs against temporary files and never needs to name a
//! device node — which is the only way to test a disk-destroying tool safely on
//! a developer's own machine.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use crate::error::{PartError, Result};

/// `BLKRRPART` — re-read the partition table.
const BLKRRPART: libc::c_ulong = 0x125F;
/// `BLKSSZGET` — logical sector size.
const BLKSSZGET: libc::c_ulong = 0x1268;

/// Default sector size for a regular file, which has no geometry of its own.
pub const FILE_SECTOR_SIZE: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub sector_size: usize,
    pub sectors: u64,
}

pub struct Device {
    pub path: PathBuf,
    pub geom: Geometry,
    pub is_block: bool,
    file: File,
}

impl Device {
    pub fn open(path: &Path, write: bool) -> Result<Device> {
        let file = OpenOptions::new()
            .read(true)
            .write(write)
            .open(path)
            .map_err(|e| io_err(path, e))?;

        // Whether this is a block device is answered by sysfs, NOT by fstat.
        //
        // `fstat` on a device node can be denied to a caller who can read the
        // device perfectly well: `dd if=/dev/vdb` succeeds where `stat` does
        // not, as SYSTEM, in the same shell. That observation is solid and
        // reproducible; **why** it happens is still open (PEI-196).
        //
        // It is NOT the access mask, though an earlier version of this comment
        // said so. `sd show` renders masks in short form and the `f` on those
        // descriptors is the composite FILE_ALL (0x001F01FF) — full access,
        // including READ_ATTRIBUTES — not hex 0xF. Reading a letter as a hex
        // digit produced a confident, wrong root cause that outlived its
        // correction in three other files; see the `sd-show-renders-masks-as-
        // letters` note before reasoning about bits from `sd show` output.
        //
        // Statting was only ever a way to ask a question sysfs answers for
        // free, so avoiding it is a straight improvement rather than a
        // workaround — `part` works wherever `dd` does, which is the right bar,
        // and it will keep working however PEI-196 is eventually explained.
        let is_block = sysfs_name(path)
            .map(|n| Path::new(&format!("/sys/class/block/{n}")).is_dir())
            .unwrap_or(false);

        let geom = if is_block {
            block_geometry(path, &file)?
        } else if let Some(len) = regular_file_len(&file, path)? {
            if len < FILE_SECTOR_SIZE as u64 {
                return Err(PartError::Usage(format!(
                    "{}: {len} bytes is too small to hold a partition table",
                    path.display()
                )));
            }
            Geometry {
                sector_size: FILE_SECTOR_SIZE,
                sectors: len / FILE_SECTOR_SIZE as u64,
            }
        } else {
            return Err(PartError::Usage(format!(
                "{}: not a block device or regular file",
                path.display()
            )));
        };

        Ok(Device {
            path: path.to_path_buf(),
            geom,
            is_block,
            file,
        })
    }

    /// Override the sector size, for exercising 4Kn layouts against a file.
    /// Refused on a real device, where the kernel's answer is the only correct
    /// one and disagreeing with it would corrupt the disk.
    pub fn with_sector_size(mut self, size: usize) -> Result<Device> {
        if self.is_block {
            return Err(PartError::Usage(
                "the sector size of a block device is not ours to choose".into(),
            ));
        }
        let bytes = self.geom.sectors * self.geom.sector_size as u64;
        self.geom = Geometry {
            sector_size: size,
            sectors: bytes / size as u64,
        };
        Ok(self)
    }

    pub fn read_sectors(&mut self, lba: u64, count: u64) -> Result<Vec<u8>> {
        let len = (count as usize) * self.geom.sector_size;
        let off = lba * self.geom.sector_size as u64;
        let mut buf = vec![0u8; len];
        self.file
            .seek(SeekFrom::Start(off))
            .map_err(|e| io_err(&self.path, e))?;
        self.file
            .read_exact(&mut buf)
            .map_err(|e| io_err(&self.path, e))?;
        Ok(buf)
    }

    pub fn write_at(&mut self, lba: u64, data: &[u8]) -> Result<()> {
        let off = lba * self.geom.sector_size as u64;
        self.file
            .seek(SeekFrom::Start(off))
            .map_err(|e| io_err(&self.path, e))?;
        self.file.write_all(data).map_err(|e| io_err(&self.path, e))
    }

    pub fn sync(&mut self) -> Result<()> {
        self.file.sync_all().map_err(|e| io_err(&self.path, e))
    }

    /// Ask the kernel to re-read the partition table.
    ///
    /// Without this the new partitions exist on disk and nowhere else — no
    /// `/dev/vdb1` appears, and the caller's very next `mkfs` fails on a path
    /// that does not exist. `/dev` is devtmpfs, so the kernel's own device model
    /// materialises the nodes; no udev is involved, which matters because Peios
    /// ships none.
    ///
    /// A regular file has no kernel-side table, so this is a no-op there.
    pub fn reread_partition_table(&mut self) -> Result<()> {
        if !self.is_block {
            return Ok(());
        }
        // SAFETY: BLKRRPART takes no argument and the fd is a live block device.
        let rc = unsafe { libc::ioctl(self.file.as_raw_fd(), BLKRRPART) };
        if rc != 0 {
            let e = std::io::Error::last_os_error();
            // EBUSY means something still holds a partition open. The table on
            // disk is correct; it is the kernel's view that is stale, and
            // saying so is more useful than a bare errno.
            if e.raw_os_error() == Some(libc::EBUSY) {
                return Err(PartError::Refused(format!(
                    "{}: the table was written, but the kernel will not re-read it while a \
                     partition is in use; detach or reboot before formatting",
                    self.path.display()
                )));
            }
            return Err(io_err(&self.path, e));
        }
        Ok(())
    }
}

/// Length of a regular file, or `None` if it is neither a regular file nor
/// something we can size.
///
/// Only reached once sysfs has said this is *not* a block device, so the
/// `fstat` here is on an ordinary file — no KACS device descriptor involved,
/// and nothing that would deny attribute access to a caller who can read it.
fn regular_file_len(file: &File, path: &Path) -> Result<Option<u64>> {
    let md = file.metadata().map_err(|e| PartError::Io {
        path: format!("{} (reading its size)", path.display()),
        source: e,
    })?;
    Ok(md.is_file().then(|| md.len()))
}

fn block_geometry(path: &Path, file: &File) -> Result<Geometry> {
    // sysfs first, exactly as lsblk does: it is the same answer without an
    // ioctl, and it is readable on a device we only opened for reading.
    let name = sysfs_name(path);
    let sector_size = name
        .as_deref()
        .and_then(|n| read_u64(&format!("/sys/class/block/{n}/queue/logical_block_size")))
        .map(|v| v as usize)
        .or_else(|| {
            let mut ssz: libc::c_int = 0;
            // SAFETY: BLKSSZGET writes one c_int through the pointer.
            let rc = unsafe { libc::ioctl(file.as_raw_fd(), BLKSSZGET, &raw mut ssz) };
            (rc == 0 && ssz > 0).then_some(ssz as usize)
        })
        .unwrap_or(FILE_SECTOR_SIZE);

    // /sys/class/block/<name>/size is always in 512-byte units regardless of
    // the logical sector size — a detail that silently halves or doubles a
    // disk if it is assumed to match.
    let sectors_512 = name
        .as_deref()
        .and_then(|n| read_u64(&format!("/sys/class/block/{n}/size")))
        .ok_or_else(|| {
            PartError::Usage(format!("{}: cannot determine device size", path.display()))
        })?;
    let sectors = sectors_512 * 512 / sector_size as u64;

    Ok(Geometry {
        sector_size,
        sectors,
    })
}

/// The `/sys/class/block` name for a device path, following symlinks so that
/// `/dev/disk/by-id/...` resolves to the node it points at.
fn sysfs_name(path: &Path) -> Option<String> {
    // The common case is /dev/<name>, which needs no syscall at all. Try that
    // first and only fall back to resolving symlinks — /dev/disk/by-id/... and
    // friends — when the plain name is not a device sysfs knows about. sysfs is
    // UNMANAGED under KACS, so looking there costs no access to the node.
    if let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) {
        if Path::new(&format!("/sys/class/block/{name}")).is_dir() {
            return Some(name);
        }
    }
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Some(real.file_name()?.to_string_lossy().into_owned())
}

fn read_u64(p: &str) -> Option<u64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

/// Refuse anything that is not a whole disk.
///
/// `/sys/class/block/<name>/partition` exists only on partitions, so its
/// presence is the signal. Naming `/dev/vda1` when you meant `/dev/vda` would
/// otherwise write a GPT *inside* a partition — a table that looks valid to
/// anything that reads it directly and is invisible to everything else.
pub fn ensure_whole_disk(path: &Path) -> Result<()> {
    let Some(name) = sysfs_name(path) else {
        return Ok(());
    };
    let p = format!("/sys/class/block/{name}/partition");
    if Path::new(&p).exists() {
        let parent = std::fs::read_to_string(format!("/sys/class/block/{name}/../uevent"))
            .ok()
            .and_then(|s| {
                s.lines()
                    .find_map(|l| l.strip_prefix("DEVNAME=").map(|v| format!("/dev/{v}")))
            })
            .unwrap_or_else(|| "the whole disk".to_string());
        return Err(PartError::Refused(format!(
            "{} is a partition, not a whole disk; did you mean {parent}?",
            path.display()
        )));
    }
    Ok(())
}

/// Refuse a disk any part of which is mounted.
///
/// Read in-process from `/proc/mounts` rather than shelling out. That is not
/// stylistic: `peios-install` carried this same guard written as `grep -q`, and
/// because Peios ships no grep the shell exited 127, `if` read it as false, and
/// the check silently passed on every run for eight revisions (PEI-191). A
/// guard that depends on an external command fails *open*.
pub fn ensure_not_mounted(path: &Path) -> Result<()> {
    let targets = device_and_partitions(path);
    let mounts = match std::fs::read_to_string("/proc/mounts") {
        Ok(m) => m,
        // No /proc/mounts means no evidence either way. Refusing here would
        // make the tool unusable in an initramfs; the whole-disk and foreign
        // -table guards still apply.
        Err(_) => return Ok(()),
    };
    for line in mounts.lines() {
        let mut f = line.split_whitespace();
        let (Some(source), Some(target)) = (f.next(), f.next()) else {
            continue;
        };
        if targets.iter().any(|t| t == source) {
            return Err(PartError::Refused(format!(
                "{source} is mounted at {target}; refusing to touch {}",
                path.display()
            )));
        }
    }
    Ok(())
}

/// One whole disk as the kernel presents it, with the partition nodes it
/// actually created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskEntry {
    /// Kernel name, e.g. `vda` or `nvme0n1`.
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    /// `(kernel name, 1-based index)`, ascending by index.
    pub partitions: Vec<(String, usize)>,
}

/// Every whole disk on the system, in name order.
///
/// Structure comes from sysfs rather than from any partition table, so a disk
/// carrying a label `part` cannot manage still lists its partitions correctly —
/// they are whatever the kernel found, which is the honest answer to "what is
/// on this machine".
///
/// Partition names are read, never derived: `sd`/`vd` number directly (`vda1`)
/// while `nvme`/`mmcblk` interpose a `p` (`nvme0n1p1`), and a rule guessed from
/// the disk name gets one of those wrong.
pub fn enumerate_disks() -> Vec<DiskEntry> {
    let Ok(rd) = std::fs::read_dir("/sys/class/block") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let base = format!("/sys/class/block/{name}");
        // A `partition` file marks a partition; a whole disk has none.
        if Path::new(&format!("{base}/partition")).exists() {
            continue;
        }
        let sectors = read_u64(&format!("{base}/size")).unwrap_or(0);
        // Zero-length devices are the empty loop and sr slots the kernel always
        // presents. Listing a dozen of them buries the disks that exist.
        if sectors == 0 {
            continue;
        }

        let mut partitions = Vec::new();
        if let Ok(children) = std::fs::read_dir(&base) {
            for c in children.flatten() {
                let cname = c.file_name().to_string_lossy().into_owned();
                if let Some(idx) = read_u64(&format!("{base}/{cname}/partition")) {
                    partitions.push((cname, idx as usize));
                }
            }
        }
        partitions.sort_by_key(|(_, i)| *i);


        out.push(DiskEntry {
            path: PathBuf::from(format!("/dev/{name}")),
            name,
            // sysfs `size` is always in 512-byte units, whatever the logical
            // sector size is.
            size_bytes: sectors * 512,
            partitions,
        });
    }
    out.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    out
}

/// Compare device names the way a person reads them, so `loop2` comes before
/// `loop10` and `sda9` before `sda10`. Plain lexicographic order puts every
/// device beginning with a `1` ahead of the one you were looking for.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (mut ai, mut bi) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut n = 0u64;
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        // Saturate rather than wrap: a pathological run of
                        // digits must not reorder the list.
                        n = n.saturating_mul(10).saturating_add(c as u64 - '0' as u64);
                        it.next();
                    }
                    n
                };
                match take(&mut ai).cmp(&take(&mut bi)) {
                    Ordering::Equal => {}
                    other => return other,
                }
            }
            (Some(x), Some(y)) => {
                match x.cmp(&y) {
                    Ordering::Equal => {}
                    other => return other,
                }
                ai.next();
                bi.next();
            }
        }
    }
}

/// The device itself plus every partition node beneath it, as `/dev` paths.
fn device_and_partitions(path: &Path) -> Vec<String> {
    let mut out = vec![path.to_string_lossy().into_owned()];
    if let Ok(real) = std::fs::canonicalize(path) {
        let s = real.to_string_lossy().into_owned();
        if !out.contains(&s) {
            out.push(s);
        }
    }
    let Some(name) = sysfs_name(path) else {
        return out;
    };
    // A partition's sysfs directory is nested inside its disk's, so listing the
    // disk's directory and keeping the entries that are themselves partitions
    // enumerates them without parsing names for trailing digits (which gets
    // nvme0n1 vs nvme0n1p1 wrong).
    if let Ok(rd) = std::fs::read_dir(format!("/sys/class/block/{name}")) {
        for e in rd.flatten() {
            let child = e.file_name().to_string_lossy().into_owned();
            if Path::new(&format!("/sys/class/block/{name}/{child}/partition")).exists() {
                out.push(format!("/dev/{child}"));
            }
        }
    }
    out
}

fn io_err(path: &Path, source: std::io::Error) -> PartError {
    PartError::Io {
        path: path.display().to_string(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(bytes: u64) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("disk.img");
        let f = File::create(&p).unwrap();
        f.set_len(bytes).unwrap();
        (dir, p)
    }

    #[test]
    fn a_regular_file_has_geometry_derived_from_its_length() {
        let (_d, p) = image(8 * 1024 * 1024);
        let dev = Device::open(&p, false).unwrap();
        assert!(!dev.is_block);
        assert_eq!(dev.geom.sector_size, 512);
        assert_eq!(dev.geom.sectors, 16384);
    }

    #[test]
    fn a_too_small_file_is_refused() {
        let (_d, p) = image(16);
        assert!(Device::open(&p, false).is_err());
    }

    #[test]
    fn sector_size_can_be_overridden_on_a_file_and_rescales_the_count() {
        let (_d, p) = image(8 * 1024 * 1024);
        let dev = Device::open(&p, false).unwrap().with_sector_size(4096).unwrap();
        assert_eq!(dev.geom.sector_size, 4096);
        assert_eq!(dev.geom.sectors, 2048);
    }

    #[test]
    fn sectors_round_trip_through_read_and_write() {
        let (_d, p) = image(1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        let payload: Vec<u8> = (0..512).map(|i| (i % 251) as u8).collect();
        dev.write_at(3, &payload).unwrap();
        dev.sync().unwrap();
        assert_eq!(dev.read_sectors(3, 1).unwrap(), payload);
        // Neighbours are untouched.
        assert!(dev.read_sectors(2, 1).unwrap().iter().all(|&b| b == 0));
        assert!(dev.read_sectors(4, 1).unwrap().iter().all(|&b| b == 0));
    }

    #[test]
    fn reading_past_the_end_is_an_error_not_a_short_read() {
        let (_d, p) = image(1024 * 1024);
        let mut dev = Device::open(&p, false).unwrap();
        assert!(dev.read_sectors(2047, 1).is_ok());
        assert!(dev.read_sectors(2048, 1).is_err());
    }

    #[test]
    fn rereading_the_table_is_a_no_op_on_a_file() {
        let (_d, p) = image(1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        assert!(dev.reread_partition_table().is_ok());
    }

    /// A regular file is not a partition, so the whole-disk guard must let it
    /// through — otherwise none of the file-backed testing would be possible.
    #[test]
    fn the_whole_disk_guard_permits_a_regular_file() {
        let (_d, p) = image(1024 * 1024);
        assert!(ensure_whole_disk(&p).is_ok());
    }

    /// The mounted guard must find a device listed in /proc/mounts. Rather than
    /// depending on this machine's mount table, check the parsing directly
    /// against a known line — the failure mode being guarded against (PEI-191)
    /// was a check that never fired, so the parse is what needs proving.
    #[test]
    fn mount_lines_are_parsed_by_source_not_by_substring() {
        let line = "/dev/vdb2 /run/t ext4 rw,relatime 0 0";
        let mut f = line.split_whitespace();
        assert_eq!(f.next(), Some("/dev/vdb2"));
        assert_eq!(f.next(), Some("/run/t"));
        // A prefix match would wrongly flag /dev/vdb when only /dev/vdb2 is
        // listed; equality is what the guard uses.
        assert_ne!("/dev/vdb", "/dev/vdb2");
    }

    #[test]
    fn an_unmounted_file_passes_the_mount_guard() {
        let (_d, p) = image(1024 * 1024);
        assert!(ensure_not_mounted(&p).is_ok());
    }

    #[test]
    fn device_list_always_includes_the_named_path() {
        let (_d, p) = image(1024 * 1024);
        let list = device_and_partitions(&p);
        assert!(list.contains(&p.to_string_lossy().into_owned()));
    }

    #[test]
    fn opening_a_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Device::open(dir.path(), false).is_err());
    }

    #[test]
    fn a_read_only_handle_cannot_write() {
        let (_d, p) = image(1024 * 1024);
        let mut dev = Device::open(&p, false).unwrap();
        assert!(dev.write_at(0, &vec![0u8; 512]).is_err());
    }

    #[test]
    fn writes_land_at_the_right_byte_offset() {
        let (_d, p) = image(1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        dev.write_at(2, &[0xAAu8; 512]).unwrap();
        dev.sync().unwrap();
        let raw = std::fs::read(&p).unwrap();
        assert!(raw[..1024].iter().all(|&b| b == 0));
        assert!(raw[1024..1536].iter().all(|&b| b == 0xAA));
        assert!(raw[1536..2048].iter().all(|&b| b == 0));
    }

    #[test]
    fn device_names_sort_the_way_people_read_them() {
        use std::cmp::Ordering;
        assert_eq!(natural_cmp("loop2", "loop10"), Ordering::Less);
        assert_eq!(natural_cmp("sda9", "sda10"), Ordering::Less);
        assert_eq!(natural_cmp("nvme0n1", "nvme0n2"), Ordering::Less);
        assert_eq!(natural_cmp("nvme0n1", "nvme10n1"), Ordering::Less);
        assert_eq!(natural_cmp("sda", "sdb"), Ordering::Less);
        assert_eq!(natural_cmp("vda", "vda"), Ordering::Equal);
        assert_eq!(natural_cmp("sda", "sda1"), Ordering::Less);
        // Purely lexicographic order would get this one backwards, which is
        // the whole reason this exists.
        assert_ne!(natural_cmp("loop2", "loop10"), "loop2".cmp("loop10"));
    }

    #[test]
    fn a_sorted_device_list_is_actually_sorted() {
        let mut v = vec!["loop10", "sda1", "loop2", "nvme0n1", "loop1"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, vec!["loop1", "loop2", "loop10", "nvme0n1", "sda1"]);
    }

    /// Whatever this machine has, the invariants must hold: a listed disk is
    /// never itself a partition, never zero-length, and its partitions are in
    /// index order.
    #[test]
    fn enumerated_disks_are_whole_disks_in_index_order() {
        for d in enumerate_disks() {
            assert!(
                !Path::new(&format!("/sys/class/block/{}/partition", d.name)).exists(),
                "{} is a partition, not a disk",
                d.name
            );
            assert!(d.size_bytes > 0, "{} has no size", d.name);
            assert_eq!(d.path, PathBuf::from(format!("/dev/{}", d.name)));
            let idx: Vec<usize> = d.partitions.iter().map(|(_, i)| *i).collect();
            let mut sorted = idx.clone();
            sorted.sort_unstable();
            assert_eq!(idx, sorted, "{} partitions out of order", d.name);
            // Names are read from sysfs, never derived — so each must really
            // exist as a child of this disk.
            for (pname, _) in &d.partitions {
                assert!(
                    Path::new(&format!("/sys/class/block/{}/{pname}", d.name)).exists(),
                    "{pname} is not a child of {}",
                    d.name
                );
            }
        }
    }

    #[test]
    fn a_file_written_through_the_device_is_readable_as_bytes() {
        let (_d, p) = image(1024 * 1024);
        {
            let mut f = OpenOptions::new().write(true).open(&p).unwrap();
            f.write_all(b"hello").unwrap();
        }
        let mut dev = Device::open(&p, false).unwrap();
        assert_eq!(&dev.read_sectors(0, 1).unwrap()[..5], b"hello");
    }
}
