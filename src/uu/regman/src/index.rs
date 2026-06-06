// The optional flat-file index (design §7).
//
// The index exists only to skip the global scan; it is never required for
// correctness and, via the cascade in `query.rs`, can never produce a wrong
// answer. It maps a folded anchor to the list of `(fragment file, byte offset)`
// locations that define it — a list, because multi-provider means an anchor can
// resolve to several files (§6).
//
// It is a single, read-only, wholesale-rebuilt map, so it is a sorted flat file
// (not SQLite — see §7.1): a sorted descriptor array + an anchor blob, queried
// with mmap + binary search, rebuilt by writing a temp file and renaming.
//
// On-disk layout (little-endian throughout):
//
//   header (40 bytes)
//     0  magic     [u8;8] = "REGMANIX"
//     8  version   u32    = 1
//     12 entries   u32      number of descriptors (distinct anchors, sorted)
//     16 desc_off  u32      byte offset of the descriptor array
//     20 locs_off  u32      byte offset of the locations array
//     24 files_off u32      byte offset of the file table
//     28 anch_off  u32      byte offset of the anchor blob
//     32 files     u32      number of provider files
//     36 reserved  u32
//   descriptors  entries × 16: anchor_off u32, anchor_len u32, loc_idx u32, loc_n u32
//   locations    Σ loc_n × 12: file_idx u32, rec_offset u64
//   file table   files × (len u32 + path bytes)
//   anchor blob  concatenated anchor bytes (descriptors point in absolutely)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use memmap2::Mmap;

use crate::corpus;
use crate::error::Result;
use crate::fragment;
use crate::scan;

const MAGIC: &[u8; 8] = b"REGMANIX";
const VERSION: u32 = 1;
const HEADER_LEN: usize = 40;
const DESC_LEN: usize = 16;
const LOC_LEN: usize = 12;

/// A resolved location of a documented record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub file: PathBuf,
    pub offset: u64,
}

/// Build the index for `dir` and write it atomically to `out`.
pub fn build(dir: &Path, out: &Path) -> Result<()> {
    // anchor -> sorted-by-construction list of (file_index, offset)
    let mut entries: BTreeMap<String, Vec<(u32, u64)>> = BTreeMap::new();
    let mut files: Vec<String> = Vec::new();

    for file in corpus::fragments(dir)? {
        let file_str = file.to_string_lossy().into_owned();
        let file_idx = if let Some(i) = files.iter().position(|f| *f == file_str) { i as u32 } else {
            files.push(file_str);
            (files.len() - 1) as u32
        };
        let text = std::fs::read_to_string(&file)?;
        let (records, _issues) = fragment::parse(&text);
        for r in records {
            entries
                .entry(r.anchor)
                .or_default()
                .push((file_idx, r.fence_offset as u64));
        }
    }

    let bytes = serialize(&entries, &files);
    write_atomic(out, &bytes)
}

fn serialize(entries: &BTreeMap<String, Vec<(u32, u64)>>, files: &[String]) -> Vec<u8> {
    let entry_count = entries.len();
    let loc_count: usize = entries.values().map(Vec::len).sum();

    let desc_off = HEADER_LEN;
    let locs_off = desc_off + entry_count * DESC_LEN;
    let files_off = locs_off + loc_count * LOC_LEN;

    // File table comes before the anchor blob; compute its length.
    let files_len: usize = files.iter().map(|f| 4 + f.len()).sum();
    let anch_off = files_off + files_len;

    let mut descriptors = Vec::with_capacity(entry_count * DESC_LEN);
    let mut locations = Vec::with_capacity(loc_count * LOC_LEN);
    let mut anchors = Vec::new();
    let mut loc_idx: u32 = 0;

    for (anchor, locs) in entries {
        let a_off = (anch_off + anchors.len()) as u32;
        let a_len = anchor.len() as u32;
        anchors.extend_from_slice(anchor.as_bytes());

        descriptors.extend_from_slice(&a_off.to_le_bytes());
        descriptors.extend_from_slice(&a_len.to_le_bytes());
        descriptors.extend_from_slice(&loc_idx.to_le_bytes());
        descriptors.extend_from_slice(&(locs.len() as u32).to_le_bytes());

        for (file_idx, offset) in locs {
            locations.extend_from_slice(&file_idx.to_le_bytes());
            locations.extend_from_slice(&offset.to_le_bytes());
            loc_idx += 1;
        }
    }

    let mut file_table = Vec::with_capacity(files_len);
    for f in files {
        file_table.extend_from_slice(&(f.len() as u32).to_le_bytes());
        file_table.extend_from_slice(f.as_bytes());
    }

    let mut out = Vec::with_capacity(anch_off + anchors.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(entry_count as u32).to_le_bytes());
    out.extend_from_slice(&(desc_off as u32).to_le_bytes());
    out.extend_from_slice(&(locs_off as u32).to_le_bytes());
    out.extend_from_slice(&(files_off as u32).to_le_bytes());
    out.extend_from_slice(&(anch_off as u32).to_le_bytes());
    out.extend_from_slice(&(files.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    debug_assert_eq!(out.len(), HEADER_LEN);

    out.extend_from_slice(&descriptors);
    out.extend_from_slice(&locations);
    out.extend_from_slice(&file_table);
    out.extend_from_slice(&anchors);
    out
}

fn write_atomic(out: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = out.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, out)?;
    Ok(())
}

/// Remove the index file, if present.
pub fn clear(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// A loaded, memory-mapped index.
pub struct Index {
    mmap: Mmap,
    files: Vec<PathBuf>,
    entries: usize,
    desc_off: usize,
    locs_off: usize,
}

/// Load the index at `path`, if it exists and is valid. A missing or malformed
/// index returns `None` — the caller falls back to a global scan.
pub fn load(path: &Path) -> Option<Index> {
    let file = std::fs::File::open(path).ok()?;
    // SAFETY: regman treats the mapping as immutable bytes; a concurrent rebuild
    // replaces the file via rename, so this mapping keeps pointing at the old
    // inode rather than seeing a torn write.
    let mmap = unsafe { Mmap::map(&file).ok()? };
    if mmap.len() < HEADER_LEN || &mmap[0..8] != MAGIC {
        return None;
    }
    if rd_u32(&mmap, 8) != VERSION {
        return None;
    }
    let entries = rd_u32(&mmap, 12) as usize;
    let desc_off = rd_u32(&mmap, 16) as usize;
    let locs_off = rd_u32(&mmap, 20) as usize;
    let files_off = rd_u32(&mmap, 24) as usize;
    let file_count = rd_u32(&mmap, 32) as usize;

    // Parse the (small) file table eagerly so lookups can resolve paths.
    let mut files = Vec::with_capacity(file_count);
    let mut at = files_off;
    for _ in 0..file_count {
        if at + 4 > mmap.len() {
            return None;
        }
        let len = rd_u32(&mmap, at) as usize;
        at += 4;
        if at + len > mmap.len() {
            return None;
        }
        files.push(PathBuf::from(String::from_utf8_lossy(&mmap[at..at + len]).into_owned()));
        at += len;
    }

    Some(Index {
        mmap,
        files,
        entries,
        desc_off,
        locs_off,
    })
}

impl Index {
    fn desc(&self, i: usize) -> (usize, usize, usize, usize) {
        let base = self.desc_off + i * DESC_LEN;
        (
            rd_u32(&self.mmap, base) as usize,     // anchor_off
            rd_u32(&self.mmap, base + 4) as usize, // anchor_len
            rd_u32(&self.mmap, base + 8) as usize, // loc_idx
            rd_u32(&self.mmap, base + 12) as usize, // loc_n
        )
    }

    fn anchor(&self, i: usize) -> &[u8] {
        let (off, len, _, _) = self.desc(i);
        &self.mmap[off..off + len]
    }

    fn locations(&self, loc_idx: usize, loc_n: usize) -> Vec<Location> {
        let mut out = Vec::with_capacity(loc_n);
        for k in 0..loc_n {
            let base = self.locs_off + (loc_idx + k) * LOC_LEN;
            let file_idx = rd_u32(&self.mmap, base) as usize;
            let offset = rd_u64(&self.mmap, base + 4);
            if let Some(file) = self.files.get(file_idx) {
                out.push(Location {
                    file: file.clone(),
                    offset,
                });
            }
        }
        out
    }

    /// First descriptor index whose anchor is `>= target`.
    fn lower_bound(&self, target: &[u8]) -> usize {
        let (mut lo, mut hi) = (0, self.entries);
        while lo < hi {
            let mid = usize::midpoint(lo, hi);
            if self.anchor(mid) < target {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// Locations for an exact anchor match.
    pub fn lookup_exact(&self, folded_anchor: &str) -> Vec<Location> {
        let target = folded_anchor.as_bytes();
        let i = self.lower_bound(target);
        if i < self.entries && self.anchor(i) == target {
            let (_, _, loc_idx, loc_n) = self.desc(i);
            self.locations(loc_idx, loc_n)
        } else {
            Vec::new()
        }
    }

    /// Locations for a key query: the key doc plus its directly-attached values.
    /// All anchors sharing the path prefix are contiguous in sorted order, so we
    /// seek the lower bound and walk forward while the prefix holds.
    pub fn lookup_key(&self, folded_path: &str) -> Vec<Location> {
        let path = folded_path.as_bytes();
        let mut out = Vec::new();
        let mut i = self.lower_bound(path);
        while i < self.entries && self.anchor(i).starts_with(path) {
            if let Ok(anchor) = std::str::from_utf8(self.anchor(i)) {
                if scan::is_under_key(anchor, folded_path) {
                    let (_, _, loc_idx, loc_n) = self.desc(i);
                    out.extend(self.locations(loc_idx, loc_n));
                }
            }
            i += 1;
        }
        out
    }
}

fn rd_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn rd_u64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("kmes.regman"),
            "\
--- machine\\system\\kmes
canonical: Machine\\System\\KMES

KMES subtree.

--- machine\\system\\kmes buffercapacity
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD

Ring buffer capacity.

--- machine\\system\\kmes maxeventsize
canonical: Machine\\System\\KMES MaxEventSize
type: REG_DWORD

Max event size.

--- machine\\system\\kmesfoo
canonical: Machine\\System\\KMESFoo

Sibling.
",
        )
        .unwrap();
        tmp
    }

    fn built() -> (tempfile::TempDir, PathBuf) {
        let tmp = corpus();
        let idx = tmp.path().join(".idx");
        build(tmp.path(), &idx).unwrap();
        (tmp, idx)
    }

    #[test]
    fn exact_lookup_round_trips_offset() {
        let (tmp, idxp) = built();
        let idx = load(&idxp).unwrap();
        let locs = idx.lookup_exact("machine\\system\\kmes buffercapacity");
        assert_eq!(locs.len(), 1);
        // The offset must really point at that record's fence.
        let text = std::fs::read_to_string(&locs[0].file).unwrap();
        assert!(text[locs[0].offset as usize..]
            .starts_with("--- machine\\system\\kmes buffercapacity"));
        drop(tmp);
    }

    #[test]
    fn exact_miss_empty() {
        let (_t, idxp) = built();
        let idx = load(&idxp).unwrap();
        assert!(idx.lookup_exact("machine\\system\\kmes nope").is_empty());
    }

    #[test]
    fn key_lookup_excludes_sibling() {
        let (_t, idxp) = built();
        let idx = load(&idxp).unwrap();
        let locs = idx.lookup_key("machine\\system\\kmes");
        // key doc + two values = 3, sibling excluded.
        assert_eq!(locs.len(), 3);
        for l in &locs {
            let text = std::fs::read_to_string(&l.file).unwrap();
            assert!(!text[l.offset as usize..].starts_with("--- machine\\system\\kmesfoo"));
        }
    }

    #[test]
    fn multi_provider_lists_all_locations() {
        let tmp = corpus();
        std::fs::write(
            tmp.path().join("other.regman"),
            "\
--- machine\\system\\kmes buffercapacity
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD

A second package documenting the same value.
",
        )
        .unwrap();
        let idxp = tmp.path().join(".idx");
        build(tmp.path(), &idxp).unwrap();
        let idx = load(&idxp).unwrap();
        let locs = idx.lookup_exact("machine\\system\\kmes buffercapacity");
        assert_eq!(locs.len(), 2);
    }

    #[test]
    fn load_missing_returns_none() {
        assert!(load(Path::new("/nonexistent/regman.idx")).is_none());
    }

    #[test]
    fn clear_removes_and_is_idempotent() {
        let (_t, idxp) = built();
        assert!(idxp.exists());
        clear(&idxp).unwrap();
        assert!(!idxp.exists());
        clear(&idxp).unwrap(); // no error second time
    }
}
