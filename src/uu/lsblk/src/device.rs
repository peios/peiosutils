// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Block-device enumeration from sysfs, and the device tree.
//!
//! `/sys/block` lists whole devices (disks, `loopN`, `srN`, `dm-N`, `mdN`);
//! partitions appear as subdirectories of their disk. Holders/slaves links
//! (`holders/`, `slaves/`) capture the stacking of dm/md/crypt devices. This
//! module reads those facts straight from sysfs — the same data source the
//! upstream tool falls back to when udev is absent, which on peios is always.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::blkid::{self, FsInfo};
use crate::config::Config;
use crate::error::{LsblkError, Result};
use crate::mountinfo::MountMap;

/// The `TYPE` column value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DeviceType {
    #[default]
    Disk,
    Part,
    Rom,
    Loop,
    Lvm,
    Crypt,
    Mpath,
    Raid,
    Dm,
}

impl DeviceType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disk => "disk",
            Self::Part => "part",
            Self::Rom => "rom",
            Self::Loop => "loop",
            Self::Lvm => "lvm",
            Self::Crypt => "crypt",
            Self::Mpath => "mpath",
            Self::Raid => "raid",
            Self::Dm => "dm",
        }
    }
}

/// One block device, with its sysfs/blkid/SD facts and its subtree.
#[derive(Clone, Debug, Default)]
pub struct Device {
    pub name: String,
    pub syspath: PathBuf,
    pub devpath: PathBuf,
    pub maj: u32,
    pub min: u32,
    pub size_bytes: u64,
    pub dtype: DeviceType,
    pub removable: bool,
    pub read_only: bool,
    pub rotational: Option<bool>,
    pub state: Option<String>,
    pub scheduler: Option<String>,
    pub model: Option<String>,
    pub vendor: Option<String>,
    pub rev: Option<String>,
    pub serial: Option<String>,
    pub transport: Option<String>,
    pub hctl: Option<String>,
    pub phy_sec: Option<u64>,
    pub log_sec: Option<u64>,
    pub min_io: Option<u64>,
    pub opt_io: Option<u64>,
    pub alignment: Option<u64>,
    pub hotplug: bool,
    pub parent_kname: Option<String>,
    pub mountpoints: Vec<String>,
    pub fs: FsInfo,
    pub children: Vec<Self>,
    /// Tree-depth, set when the tree is assembled (drives indentation).
    pub depth: usize,
}

/// A flat sysfs record, before the tree is assembled.
struct Record {
    dev: Device,
    is_partition: bool,
    holders: Vec<String>,
    slaves: Vec<String>,
}

/// Enumerate every block device and return the forward (or, with `-s`, inverse)
/// tree of roots. Each returned `Device` carries its full subtree.
pub fn enumerate(config: &Config) -> Result<Vec<Device>> {
    let root = config.root.as_path();
    let block = root.join("sys/block");
    let entries = std::fs::read_dir(&block)
        .map_err(|e| LsblkError::System(format!("cannot read {}: {e}", block.display())))?;

    let mounts = MountMap::read(root);
    let mut records: BTreeMap<String, Record> = BTreeMap::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // /sys/block/<name> is a symlink into the device tree; canonicalize so
        // the holders/slaves of dm/md devices resolve consistently. Skip that
        // for a fixture --sysroot, whose symlinks (if any) would escape it.
        let syspath = if config.is_real_root() {
            std::fs::canonicalize(entry.path()).unwrap_or_else(|_| entry.path())
        } else {
            entry.path()
        };
        collect_record(&name, &syspath, None, false, root, &mounts, &mut records);

        // Partitions live as subdirectories carrying a `partition` file.
        if let Ok(children) = std::fs::read_dir(&syspath) {
            for sub in children.flatten() {
                let subpath = sub.path();
                if subpath.join("partition").exists() {
                    let pname = sub.file_name().to_string_lossy().into_owned();
                    collect_record(&pname, &subpath, Some(&name), true, root, &mounts, &mut records);
                }
            }
        }
    }

    let inverse = config.inverse;
    Ok(build_tree(&records, inverse))
}

/// Read one device's sysfs facts into `records`.
fn collect_record(
    name: &str,
    syspath: &Path,
    parent_disk: Option<&str>,
    is_partition: bool,
    root: &Path,
    mounts: &MountMap,
    records: &mut BTreeMap<String, Record>,
) {
    if records.contains_key(name) {
        return;
    }

    let (maj, min) = read_dev(syspath).unwrap_or((0, 0));
    let size_bytes = read_u64(syspath, "size").unwrap_or(0).saturating_mul(512);
    // The device node lives under <root>/dev; for a real run that is /dev.
    let devpath = root.join("dev").join(name);

    let dtype = classify(name, syspath, is_partition);
    let holders = list_links(syspath, "holders");
    let slaves = list_links(syspath, "slaves");

    // The hardware columns hang off the `device` symlink (absent for synthetic
    // devices like dm/loop, which is fine — those columns are then empty).
    let model = read_trim(syspath, "device/model");
    let vendor = read_trim(syspath, "device/vendor");
    let rev = read_trim(syspath, "device/rev");
    let serial = read_trim(syspath, "device/serial");

    let transport = read_transport(syspath);
    let removable = read_bool(syspath, "removable");
    // No udev: approximate HOTPLUG by the removable flag or a hotpluggable bus.
    let hotplug = removable || transport.as_deref() == Some("usb");

    let dev = Device {
        name: name.to_string(),
        syspath: syspath.to_path_buf(),
        devpath: devpath.clone(),
        maj,
        min,
        size_bytes,
        dtype,
        removable,
        read_only: read_bool(syspath, "ro"),
        rotational: read_u64(syspath, "queue/rotational").map(|v| v != 0),
        state: read_trim(syspath, "device/state"),
        scheduler: read_scheduler(syspath),
        model,
        vendor,
        rev,
        serial,
        transport,
        hctl: read_hctl(syspath),
        // queue/* topology lives on the whole disk; a partition inherits its
        // parent's. alignment_offset is per-device.
        phy_sec: read_queue_u64(syspath, "physical_block_size"),
        log_sec: read_queue_u64(syspath, "logical_block_size"),
        min_io: read_queue_u64(syspath, "minimum_io_size"),
        opt_io: read_queue_u64(syspath, "optimal_io_size"),
        alignment: read_u64(syspath, "alignment_offset"),
        hotplug,
        parent_kname: parent_disk.map(str::to_string),
        mountpoints: mounts.lookup(maj, min),
        fs: blkid::probe(&devpath).unwrap_or_default(),
        children: Vec::new(),
        depth: 0,
    };

    records.insert(
        name.to_string(),
        Record { dev, is_partition, holders, slaves },
    );
}

/// Assemble the tree from the flat records. Forward: roots are physical wholes
/// (no slaves, not a partition); children are partitions then holders. Inverse
/// (`-s`): roots are leaves (nothing stacked on them); children are slaves and,
/// for a partition, its parent disk.
fn build_tree(records: &BTreeMap<String, Record>, inverse: bool) -> Vec<Device> {
    // Map disk -> its partition knames (sysfs nesting, recorded as parent_kname).
    let mut partitions_of: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (kname, rec) in records {
        if let Some(parent) = rec.dev.parent_kname.as_deref() {
            partitions_of.entry(parent).or_default().push(kname);
        }
    }

    let roots: Vec<&str> = records
        .iter()
        .filter(|(_, rec)| {
            if inverse {
                rec.holders.is_empty() && !is_parent_disk(&rec.dev.name, &partitions_of)
            } else {
                !rec.is_partition && rec.slaves.is_empty()
            }
        })
        .map(|(k, _)| k.as_str())
        .collect();

    roots
        .into_iter()
        .map(|root| assemble(root, records, &partitions_of, inverse, 0, &mut Vec::new()))
        .collect()
}

fn is_parent_disk(name: &str, partitions_of: &BTreeMap<&str, Vec<&str>>) -> bool {
    partitions_of.get(name).is_some_and(|v| !v.is_empty())
}

/// Build the subtree rooted at `kname`. `chain` guards against revisiting a node
/// within the current branch (multipath/stacked dm can otherwise loop).
fn assemble(
    kname: &str,
    records: &BTreeMap<String, Record>,
    partitions_of: &BTreeMap<&str, Vec<&str>>,
    inverse: bool,
    depth: usize,
    chain: &mut Vec<String>,
) -> Device {
    let rec = &records[kname];
    let mut dev = rec.dev.clone();
    dev.depth = depth;

    if chain.iter().any(|c| c == kname) {
        return dev; // cycle guard: stop without recursing
    }
    chain.push(kname.to_string());

    let child_names: Vec<String> = if inverse {
        let mut names: Vec<String> = rec.slaves.clone();
        if let Some(parent) = dev.parent_kname.as_deref() {
            names.push(parent.to_string());
        }
        names
    } else {
        let mut names: Vec<String> = partitions_of
            .get(kname)
            .map(|v| v.iter().map(|s| (*s).to_string()).collect())
            .unwrap_or_default();
        names.extend(rec.holders.iter().cloned());
        names
    };

    for child in child_names {
        if records.contains_key(&child) {
            dev.children
                .push(assemble(&child, records, partitions_of, inverse, depth + 1, chain));
        }
    }

    chain.pop();
    dev
}

/// Classify the `TYPE` column from the device name and sysfs.
fn classify(name: &str, syspath: &Path, is_partition: bool) -> DeviceType {
    if is_partition {
        return DeviceType::Part;
    }
    if name.starts_with("loop") {
        return DeviceType::Loop;
    }
    if name.starts_with("sr") {
        return DeviceType::Rom;
    }
    if name.starts_with("dm-") {
        // Refine via the dm uuid prefix (LVM-/CRYPT-/mpath-/…).
        if let Some(uuid) = read_trim(syspath, "dm/uuid") {
            let lower = uuid.to_ascii_lowercase();
            if lower.starts_with("lvm-") {
                return DeviceType::Lvm;
            }
            if lower.starts_with("crypt-") {
                return DeviceType::Crypt;
            }
            if lower.starts_with("mpath-") {
                return DeviceType::Mpath;
            }
        }
        return DeviceType::Dm;
    }
    if name.starts_with("md") {
        return DeviceType::Raid;
    }
    DeviceType::Disk
}

// --- sysfs read helpers -----------------------------------------------------

/// `dev` holds `MAJ:MIN`.
fn read_dev(syspath: &Path) -> Option<(u32, u32)> {
    let s = read_trim(syspath, "dev")?;
    let (maj, min) = s.split_once(':')?;
    Some((maj.parse().ok()?, min.parse().ok()?))
}

fn read_trim(syspath: &Path, rel: &str) -> Option<String> {
    let data = std::fs::read_to_string(syspath.join(rel)).ok()?;
    let trimmed = data.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn read_u64(syspath: &Path, rel: &str) -> Option<u64> {
    read_trim(syspath, rel)?.parse().ok()
}

/// Read a `queue/<rel>` topology value. A partition has no `queue/` of its own,
/// so fall back to the parent disk's (the partition's sysfs dir is nested under
/// the disk's).
fn read_queue_u64(syspath: &Path, rel: &str) -> Option<u64> {
    let own = format!("queue/{rel}");
    read_u64(syspath, &own).or_else(|| {
        let parent = syspath.parent()?;
        read_u64(parent, &own)
    })
}

/// A sysfs boolean flag file (`removable`, `ro`): present and `1` ⇒ true.
fn read_bool(syspath: &Path, rel: &str) -> bool {
    read_trim(syspath, rel).as_deref() == Some("1")
}

/// `queue/scheduler` lists every scheduler with the active one in `[brackets]`.
fn read_scheduler(syspath: &Path) -> Option<String> {
    let s = read_trim(syspath, "queue/scheduler")?;
    for tok in s.split_whitespace() {
        if let Some(active) = tok.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
            return Some(active.to_string());
        }
    }
    Some(s)
}

/// `holders/` and `slaves/` are directories of symlinks named for the stacked
/// devices; we only need the link names.
fn list_links(syspath: &Path, rel: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(dir) = std::fs::read_dir(syspath.join(rel)) {
        for e in dir.flatten() {
            out.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    out.sort();
    out
}

/// `HCTL` (`Host:Channel:Target:Lun`) is the name of the scsi_device directory
/// under `device/scsi_device/`.
fn read_hctl(syspath: &Path) -> Option<String> {
    let dir = std::fs::read_dir(syspath.join("device/scsi_device")).ok()?;
    dir.flatten()
        .next()
        .map(|e| e.file_name().to_string_lossy().into_owned())
}

/// Derive `TRAN` by walking the device's subsystem ancestry — the same heuristic
/// the upstream tool uses without udev. We look for a recognizable bus among the
/// `subsystem` links along `device/…/`.
fn read_transport(syspath: &Path) -> Option<String> {
    // NVMe namespaces expose an `nvme` node directly.
    if syspath.join("device/nvme").exists() || syspath.to_string_lossy().contains("/nvme") {
        return Some("nvme".to_string());
    }
    // Walk up from `device`, reading each level's `subsystem` symlink basename.
    let mut cur = syspath.join("device");
    for _ in 0..8 {
        if let Ok(target) = std::fs::read_link(cur.join("subsystem")) {
            if let Some(bus) = target.file_name().and_then(|n| n.to_str()) {
                match bus {
                    "usb" => return Some("usb".to_string()),
                    "scsi" => { /* keep walking; refined below */ }
                    "nvme" => return Some("nvme".to_string()),
                    "virtio" => return Some("virtio".to_string()),
                    "mmc" => return Some("mmc".to_string()),
                    _ => {}
                }
            }
        }
        // Distinguish sata/sas/ata under the scsi bus via a sibling marker.
        if cur.join("ata_device").exists() || cur.join("../ata_port").exists() {
            return Some("sata".to_string());
        }
        match cur.parent() {
            Some(p) if p.starts_with("/sys") => cur = p.to_path_buf(),
            _ => break,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, OutputMode};

    fn write(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// A `Config` rooted at a fixture tree — the whole point of `--sysroot`.
    fn fixture_config(root: &Path) -> Config {
        Config {
            columns: crate::column::DEFAULT.to_vec(),
            mode: OutputMode::Tree,
            ascii: false,
            bytes: false,
            noheadings: false,
            paths: false,
            nodeps: false,
            all: false,
            inverse: false,
            include: None,
            exclude: Vec::new(),
            root: root.to_path_buf(),
            idlink: BTreeMap::new(),
            operands: Vec::new(),
            tree_column: crate::column::Column::Name,
            merge: false,
            dedup: None,
            sort: None,
            shell: false,
            width: None,
        }
    }

    #[test]
    fn enumerate_against_a_sysroot_fixture() {
        // A fake sysfs: one disk `sdz` (8:0, 1MiB) with one partition `sdz1`.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let disk = root.join("sys/block/sdz");
        write(&disk.join("dev"), "8:0\n");
        write(&disk.join("size"), "2048\n"); // 2048 * 512 = 1 MiB
        write(&disk.join("ro"), "0\n");
        write(&disk.join("removable"), "0\n");
        write(&disk.join("queue/rotational"), "1\n");
        write(&disk.join("queue/logical_block_size"), "512\n");
        write(&disk.join("queue/physical_block_size"), "4096\n");
        let part = disk.join("sdz1");
        write(&part.join("partition"), "1\n");
        write(&part.join("dev"), "8:1\n");
        write(&part.join("size"), "1024\n"); // 512 KiB
        write(&root.join("proc/self/mountinfo"), "");

        let config = fixture_config(root);
        let tree = enumerate(&config).unwrap();

        assert_eq!(tree.len(), 1);
        let disk = &tree[0];
        assert_eq!(disk.name, "sdz");
        assert_eq!(disk.maj, 8);
        assert_eq!(disk.size_bytes, 2048 * 512);
        assert_eq!(disk.dtype, DeviceType::Disk);
        assert_eq!(disk.children.len(), 1);

        let part = &disk.children[0];
        assert_eq!(part.name, "sdz1");
        assert_eq!(part.dtype, DeviceType::Part);
        assert_eq!(part.size_bytes, 1024 * 512);
        assert_eq!(part.parent_kname.as_deref(), Some("sdz"));
        // The partition inherits the disk's physical_block_size topology.
        assert_eq!(part.phy_sec, Some(4096));
    }
}
