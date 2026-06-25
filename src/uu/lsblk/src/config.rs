// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Parsed invocation: selected columns, output mode, and device filters.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use clap::ArgMatches;

use crate::cli::opt;
use crate::column::{self, Column};
use crate::device::Device;
use crate::error::{LsblkError, Result};

/// How the device list is rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputMode {
    /// Default indented tree.
    Tree,
    /// Flat list (`-l`).
    List,
    /// JSON (`-J`).
    Json,
    /// `KEY="value"` pairs (`-P`).
    Pairs,
    /// Raw space-separated (`-r`).
    Raw,
}

pub struct Config {
    pub columns: Vec<Column>,
    pub mode: OutputMode,
    pub ascii: bool,
    pub bytes: bool,
    pub noheadings: bool,
    pub paths: bool,
    pub nodeps: bool,
    pub all: bool,
    pub inverse: bool,
    pub include: Option<Vec<u32>>,
    pub exclude: Vec<u32>,
    /// Filesystem root for all reads; `/` for a normal run, a fixture dir under
    /// `--sysroot` for tests.
    pub root: PathBuf,
    /// `MAJ:MIN` → best `/dev/disk/by-id` link, built once per run from `root`.
    pub idlink: BTreeMap<(u32, u32), String>,
    /// Device operands (`lsblk /dev/sda …`); empty means "all devices".
    pub operands: Vec<String>,
    /// Column the tree glyphs attach to (`--tree=COL`); `NAME` by default.
    pub tree_column: Column,
    /// `-M`: collapse repeated holder subtrees (multipath/RAID readability).
    pub merge: bool,
    /// `-E COL`: drop rows whose `COL` value duplicates an earlier one.
    pub dedup: Option<Column>,
    /// `-x COL`: sort siblings by `COL`.
    pub sort: Option<Column>,
    /// `-y`: render column keys shell-safe (`MAJ:MIN` → `MAJ_MIN`).
    pub shell: bool,
    /// `-w NUM`: truncate table rows to this width.
    pub width: Option<usize>,
}

impl Config {
    pub fn from_matches(m: &ArgMatches) -> Result<Self> {
        let columns = select_columns(m)?;
        let root = m
            .get_one::<String>(opt::SYSROOT)
            .map_or_else(|| PathBuf::from("/"), PathBuf::from);
        let operands = m
            .get_many::<String>(opt::DEVICE)
            .map(|v| v.cloned().collect())
            .unwrap_or_default();
        let idlink = build_idlink(&root);

        // `-T`/`--tree[=COL]` forces tree mode (overriding -l) and chooses the
        // column the glyphs attach to.
        let tree_value = m.get_one::<String>(opt::TREE);
        let force_tree = tree_value.is_some();
        let tree_column = match tree_value {
            Some(s) => Column::parse(s)?,
            None => Column::Name,
        };

        let mode = if force_tree {
            OutputMode::Tree
        } else if m.get_flag(opt::JSON) {
            OutputMode::Json
        } else if m.get_flag(opt::PAIRS) {
            OutputMode::Pairs
        } else if m.get_flag(opt::RAW) {
            OutputMode::Raw
        } else if m.get_flag(opt::LIST) {
            OutputMode::List
        } else {
            OutputMode::Tree
        };

        let width = m
            .get_one::<String>(opt::WIDTH)
            .map(|s| {
                s.parse::<usize>()
                    .map_err(|_| LsblkError::Usage(format!("invalid width: {s}")))
            })
            .transpose()?;

        Ok(Self {
            columns,
            mode,
            ascii: m.get_flag(opt::ASCII),
            bytes: m.get_flag(opt::BYTES),
            noheadings: m.get_flag(opt::NOHEADINGS),
            paths: m.get_flag(opt::PATHS),
            nodeps: m.get_flag(opt::NODEPS),
            all: m.get_flag(opt::ALL),
            inverse: m.get_flag(opt::INVERSE),
            include: parse_majors(m, opt::INCLUDE)?,
            exclude: parse_majors(m, opt::EXCLUDE)?.unwrap_or_default(),
            root,
            idlink,
            operands,
            tree_column,
            merge: m.get_flag(opt::MERGE),
            dedup: m
                .get_one::<String>(opt::DEDUP)
                .map(|s| Column::parse(s))
                .transpose()?,
            sort: m
                .get_one::<String>(opt::SORT)
                .map(|s| Column::parse(s))
                .transpose()?,
            shell: m.get_flag(opt::SHELL),
            width,
        })
    }

    /// True for a normal (`/`) run, false under a `--sysroot` fixture.
    pub fn is_real_root(&self) -> bool {
        self.root == Path::new("/")
    }

    /// Prune the tree by the device filters: major include/exclude, the
    /// hide-empty default, and `-d`/`--nodeps`. With device operands, re-root the
    /// output at the matching subtrees first.
    pub fn apply_filters(&self, tree: &mut Vec<Device>) {
        if !self.operands.is_empty() {
            *tree = self.select_operands(tree);
        }
        tree.retain_mut(|dev| self.keep_subtree(dev));
        if self.nodeps {
            for dev in tree.iter_mut() {
                dev.children.clear();
            }
        }
        // `-M` collapses duplicate holder subtrees (a multipath dm device shows
        // once instead of under every path); `-E` is the same dedup keyed on an
        // arbitrary column.
        if self.merge {
            dedup_by(tree, &|d| d.name.clone(), &mut HashSet::new());
        }
        if let Some(col) = self.dedup {
            dedup_by(tree, &|d| col.value(d, self).unwrap_or_default(), &mut HashSet::new());
        }
        if let Some(col) = self.sort {
            sort_tree(tree, col, self);
        }
    }

    /// Collect the subtrees whose device matches a `[DEVICE...]` operand. An
    /// operand matches by basename, by full `/dev` path, or — when it names an
    /// existing node — by its `MAJ:MIN`. A matched partition becomes a new root.
    fn select_operands(&self, tree: &[Device]) -> Vec<Device> {
        // Pre-resolve operands to (basename, optional maj:min).
        let targets: Vec<(String, Option<(u32, u32)>)> = self
            .operands
            .iter()
            .map(|op| {
                let base = Path::new(op)
                    .file_name()
                    .map_or_else(|| op.clone(), |b| b.to_string_lossy().into_owned());
                (base, stat_majmin(op))
            })
            .collect();

        let mut out = Vec::new();
        collect_matching(tree, &targets, &mut out);
        out
    }

    /// Keep `dev` (recursively pruning its children), or `false` to drop it.
    fn keep_subtree(&self, dev: &mut Device) -> bool {
        dev.children.retain_mut(|child| self.keep_subtree(child));
        self.included(dev)
    }

    fn included(&self, dev: &Device) -> bool {
        if let Some(inc) = &self.include {
            if !inc.contains(&dev.maj) {
                return false;
            }
        } else {
            let exclude = self.effective_exclude();
            if exclude.contains(&dev.maj) {
                return false;
            }
        }
        // Hide empty (zero-size) devices unless `-a`, but never hide one that
        // still has visible children.
        if !self.all && dev.size_bytes == 0 && dev.children.is_empty() {
            return false;
        }
        true
    }

    /// `-a` clears exclusions; an explicit `-e` replaces the default; otherwise
    /// RAM disks (major 1) are hidden, matching upstream.
    fn effective_exclude(&self) -> Vec<u32> {
        if self.all {
            Vec::new()
        } else if self.exclude.is_empty() {
            vec![1]
        } else {
            self.exclude.clone()
        }
    }
}

/// Drop subtrees whose key (by `key`) was already seen, depth-first. Keeps the
/// first occurrence; used by both `-M` (key = kname) and `-E` (key = a column).
fn dedup_by<F: Fn(&Device) -> String>(
    tree: &mut Vec<Device>,
    key: &F,
    seen: &mut HashSet<String>,
) {
    tree.retain(|d| seen.insert(key(d)));
    for d in tree.iter_mut() {
        dedup_by(&mut d.children, key, seen);
    }
}

/// Sort each sibling list by `col`. Numeric columns (SIZE, MAJ:MIN) sort by
/// their underlying value, not the formatted string.
fn sort_tree(tree: &mut [Device], col: Column, config: &Config) {
    tree.sort_by(|a, b| cmp_by_col(a, b, col, config));
    for d in tree.iter_mut() {
        sort_tree(&mut d.children, col, config);
    }
}

fn cmp_by_col(a: &Device, b: &Device, col: Column, config: &Config) -> std::cmp::Ordering {
    match col {
        Column::Size => a.size_bytes.cmp(&b.size_bytes),
        Column::MajMin => (a.maj, a.min).cmp(&(b.maj, b.min)),
        _ => col.value(a, config).cmp(&col.value(b, config)),
    }
}

/// Whether a device matches any operand target.
fn matches_target(dev: &Device, targets: &[(String, Option<(u32, u32)>)]) -> bool {
    targets.iter().any(|(base, majmin)| {
        dev.name == *base
            || dev.devpath.to_string_lossy() == *base
            || *majmin == Some((dev.maj, dev.min))
    })
}

/// Walk the tree, pushing each matching device (with its subtree) as a new root.
fn collect_matching(
    tree: &[Device],
    targets: &[(String, Option<(u32, u32)>)],
    out: &mut Vec<Device>,
) {
    for dev in tree {
        if matches_target(dev, targets) {
            out.push(dev.clone());
        } else {
            collect_matching(&dev.children, targets, out);
        }
    }
}

/// `stat(2)` a path and return its node's device number as `(maj, min)`, or
/// `None` if it doesn't exist / isn't a device node.
fn stat_majmin(path: &str) -> Option<(u32, u32)> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).ok()?;
    let rdev = meta.rdev();
    if rdev == 0 {
        return None;
    }
    Some((libc::major(rdev), libc::minor(rdev)))
}

/// `MAJ:MIN` → best `/dev/disk/by-id` link, built from `<root>/dev/disk/by-id`.
///
/// Sourced purely from the symlink farm (NOT a `/run/udev/data` parser — that
/// fast-path waits on peios-udev). Empty until a device manager populates the
/// farm, at which point `ID-LINK` lights up. "Best" = the shortest name.
fn build_idlink(root: &Path) -> BTreeMap<(u32, u32), String> {
    use std::os::unix::fs::MetadataExt;
    // TODO(substrate): once peios-udev exists and its db format is fixed, add a
    // /run/udev/data fast-path (keyed ID_* lookups, the way upstream lsblk does)
    // for ID/ID-LINK and a canonical WWN. Until then these come only from the
    // by-id symlink farm below, which is empty until a device manager populates
    // /dev/disk/by-id. Tracked as a standalone task.
    let mut map: BTreeMap<(u32, u32), String> = BTreeMap::new();
    let Ok(dir) = std::fs::read_dir(root.join("dev/disk/by-id")) else {
        return map;
    };
    for entry in dir.flatten() {
        // metadata() follows the symlink to the device node; rdev is its number.
        let Ok(meta) = std::fs::metadata(entry.path()) else {
            continue;
        };
        let rdev = meta.rdev();
        let (maj, min) = (libc::major(rdev), libc::minor(rdev));
        let name = entry.file_name().to_string_lossy().into_owned();
        map.entry((maj, min))
            .and_modify(|cur| {
                if name.len() < cur.len() {
                    cur.clone_from(&name);
                }
            })
            .or_insert(name);
    }
    map
}

/// `-o` wins; then `-O` (all), `-f` (fs), `-m` (perms); else the default set.
fn select_columns(m: &ArgMatches) -> Result<Vec<Column>> {
    if let Some(spec) = m.get_one::<String>(opt::OUTPUT) {
        column::parse_list(spec)
    } else if m.get_flag(opt::OUTPUT_ALL) {
        Ok(column::ALL.to_vec())
    } else if m.get_flag(opt::FS) {
        Ok(column::FS.to_vec())
    } else if m.get_flag(opt::PERMS) {
        Ok(column::PERMS.to_vec())
    } else {
        Ok(column::DEFAULT.to_vec())
    }
}

/// Parse a comma-separated major-number list (`-e`/`-I`).
fn parse_majors(m: &ArgMatches, id: &str) -> Result<Option<Vec<u32>>> {
    let Some(spec) = m.get_one::<String>(id) else {
        return Ok(None);
    };
    let majors = spec
        .split(',')
        .filter(|t| !t.trim().is_empty())
        .map(|t| {
            t.trim()
                .parse::<u32>()
                .map_err(|_| LsblkError::Usage(format!("invalid major number: {t}")))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(majors))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> Config {
        Config {
            columns: column::DEFAULT.to_vec(),
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
            root: PathBuf::from("/"),
            idlink: BTreeMap::new(),
            operands: Vec::new(),
            tree_column: Column::Name,
            merge: false,
            dedup: None,
            sort: None,
            shell: false,
            width: None,
        }
    }

    fn dev(name: &str, size: u64, children: Vec<Device>) -> Device {
        Device { name: name.to_string(), size_bytes: size, children, ..Default::default() }
    }

    #[test]
    fn merge_shows_a_shared_holder_once() {
        // Two paths (sda, sdb) each holding the same dm0 — multipath.
        let mut tree = vec![
            dev("sda", 100, vec![dev("dm0", 100, vec![])]),
            dev("sdb", 100, vec![dev("dm0", 100, vec![])]),
        ];
        dedup_by(&mut tree, &|d| d.name.clone(), &mut HashSet::new());
        // dm0 is kept under the first path only.
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[1].children.len(), 0);
    }

    #[test]
    fn sort_orders_siblings_by_size_numerically() {
        let mut tree = vec![dev("big", 2000, vec![]), dev("small", 30, vec![]), dev("mid", 500, vec![])];
        sort_tree(&mut tree, Column::Size, &base_config());
        let order: Vec<&str> = tree.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(order, ["small", "mid", "big"]);
    }
}
