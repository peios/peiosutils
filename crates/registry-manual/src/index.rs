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
// with an owned snapshot + binary search, rebuilt by writing a temp file and renaming.
//
// Wildcard anchors (see `pattern`) do not sort adjacent to the concrete paths
// they answer for, so the binary search alone cannot find them. They are few,
// so the index carries a second small array — the descriptor indices of the
// wildcard-bearing anchors — which a lookup scans linearly after the search.
// The sorted descriptor array, and the contiguity `lookup_key` depends on, are
// untouched by this.
//
// On-disk layout (little-endian throughout):
//
//   header (48 bytes)
//     0  magic     [u8;8] = "REGMANIX"
//     8  version   u32    = 2
//     12 entries   u32      number of descriptors (distinct anchors, sorted)
//     16 desc_off  u32      byte offset of the descriptor array
//     20 locs_off  u32      byte offset of the locations array
//     24 files_off u32      byte offset of the file table
//     28 anch_off  u32      byte offset of the anchor blob
//     32 files     u32      number of provider files
//     36 wild_off  u32      byte offset of the wildcard descriptor-index array
//     40 wild_n    u32      number of wildcard descriptors
//     44 reserved  u32
//   descriptors  entries × 16: anchor_off u32, anchor_len u32, loc_idx u32, loc_n u32
//   locations    Σ loc_n × 12: file_idx u32, rec_offset u64
//   file table   files × (len u32 + path bytes)
//   wildcards    wild_n × 4: descriptor index u32 (ascending)
//   anchor blob  concatenated anchor bytes (descriptors point in absolutely)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use std::io::Write;

use crate::corpus;
use crate::error::Result;
use crate::fragment;
use crate::pattern;
use crate::scan;

const MAGIC: &[u8; 8] = b"REGMANIX";
const VERSION: u32 = 2;
const HEADER_LEN: usize = 48;
const DESC_LEN: usize = 16;
const LOC_LEN: usize = 12;
const WILD_LEN: usize = 4;

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
        let file_idx = if let Some(i) = files.iter().position(|f| *f == file_str) {
            i as u32
        } else {
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
    let wild_count = entries.keys().filter(|a| pattern::has_wildcard(a)).count();

    let desc_off = HEADER_LEN;
    let locs_off = desc_off + entry_count * DESC_LEN;
    let files_off = locs_off + loc_count * LOC_LEN;

    // File table and wildcard array both come before the anchor blob.
    let files_len: usize = files.iter().map(|f| 4 + f.len()).sum();
    let wild_off = files_off + files_len;
    let anch_off = wild_off + wild_count * WILD_LEN;

    let mut descriptors = Vec::with_capacity(entry_count * DESC_LEN);
    let mut locations = Vec::with_capacity(loc_count * LOC_LEN);
    let mut wildcards = Vec::with_capacity(wild_count * WILD_LEN);
    let mut anchors = Vec::new();
    let mut loc_idx: u32 = 0;

    for (i, (anchor, locs)) in entries.iter().enumerate() {
        let a_off = (anch_off + anchors.len()) as u32;
        let a_len = anchor.len() as u32;
        anchors.extend_from_slice(anchor.as_bytes());

        descriptors.extend_from_slice(&a_off.to_le_bytes());
        descriptors.extend_from_slice(&a_len.to_le_bytes());
        descriptors.extend_from_slice(&loc_idx.to_le_bytes());
        descriptors.extend_from_slice(&(locs.len() as u32).to_le_bytes());

        if pattern::has_wildcard(anchor) {
            wildcards.extend_from_slice(&(i as u32).to_le_bytes());
        }

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
    out.extend_from_slice(&(wild_off as u32).to_le_bytes());
    out.extend_from_slice(&(wild_count as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    debug_assert_eq!(out.len(), HEADER_LEN);

    out.extend_from_slice(&descriptors);
    out.extend_from_slice(&locations);
    out.extend_from_slice(&file_table);
    out.extend_from_slice(&wildcards);
    out.extend_from_slice(&anchors);
    out
}

fn write_atomic(out: &Path, bytes: &[u8]) -> Result<()> {
    let parent = out
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    // NamedTempFile creates exclusively and removes the staging file on failure.
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.persist(out).map_err(|error| error.error)?;
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

/// An immutable, validated snapshot of an index.
pub struct Index {
    bytes: Vec<u8>,
    files: Vec<PathBuf>,
    entries: usize,
    desc_off: usize,
    locs_off: usize,
    wild_off: usize,
    wild_n: usize,
}

/// Load the index at `path`, if it exists and is valid. A missing or malformed
/// index returns `None` — the caller falls back to a global scan.
pub fn load(path: &Path) -> Option<Index> {
    decode(std::fs::read(path).ok()?)
}

fn decode(bytes: Vec<u8>) -> Option<Index> {
    if bytes.len() < HEADER_LEN || &bytes[..8] != MAGIC || rd_u32(&bytes, 8) != VERSION {
        return None;
    }
    let entries = rd_u32(&bytes, 12) as usize;
    let desc_off = rd_u32(&bytes, 16) as usize;
    let locs_off = rd_u32(&bytes, 20) as usize;
    let files_off = rd_u32(&bytes, 24) as usize;
    let anch_off = rd_u32(&bytes, 28) as usize;
    let file_count = rd_u32(&bytes, 32) as usize;
    let wild_off = rd_u32(&bytes, 36) as usize;
    let wild_n = rd_u32(&bytes, 40) as usize;
    if desc_off != HEADER_LEN
        || desc_off.checked_add(entries.checked_mul(DESC_LEN)?)? != locs_off
        || locs_off > files_off
        || !(files_off - locs_off).is_multiple_of(LOC_LEN)
        || files_off > wild_off
        || wild_off.checked_add(wild_n.checked_mul(WILD_LEN)?)? != anch_off
        || anch_off > bytes.len()
        || file_count > (wild_off - files_off) / 4
    {
        return None;
    }

    let mut files = Vec::new();
    let mut at = files_off;
    for _ in 0..file_count {
        let length_end = at.checked_add(4)?;
        if length_end > wild_off {
            return None;
        }
        let len = rd_u32(&bytes, at) as usize;
        let end = length_end.checked_add(len)?;
        if end > wild_off {
            return None;
        }
        files.push(PathBuf::from(
            std::str::from_utf8(&bytes[length_end..end]).ok()?,
        ));
        at = end;
    }
    if at != wild_off {
        return None;
    }

    let index = Index {
        bytes,
        files,
        entries,
        desc_off,
        locs_off,
        wild_off,
        wild_n,
    };
    index.validate_records(anch_off, (files_off - locs_off) / LOC_LEN)?;
    Some(index)
}

impl Index {
    // Validate every offset before the infallible lookup helpers can use it.
    fn validate_records(&self, anch_off: usize, location_count: usize) -> Option<()> {
        let mut previous: Option<&[u8]> = None;
        let mut wildcard = 0;
        for i in 0..self.entries {
            let (offset, length, location, count) = self.desc(i);
            if offset < anch_off
                || offset.checked_add(length)? > self.bytes.len()
                || location.checked_add(count)? > location_count
            {
                return None;
            }
            let anchor = self.anchor(i);
            let text = std::str::from_utf8(anchor).ok()?;
            if previous.is_some_and(|value| value >= anchor) {
                return None;
            }
            previous = Some(anchor);
            if pattern::has_wildcard(text) {
                if wildcard >= self.wild_n || self.wild(wildcard) != i {
                    return None;
                }
                wildcard += 1;
            }
        }
        if wildcard != self.wild_n {
            return None;
        }
        for i in 0..location_count {
            if rd_u32(&self.bytes, self.locs_off + i * LOC_LEN) as usize >= self.files.len() {
                return None;
            }
        }
        Some(())
    }

    fn desc(&self, i: usize) -> (usize, usize, usize, usize) {
        let base = self.desc_off + i * DESC_LEN;
        (
            rd_u32(&self.bytes, base) as usize,      // anchor_off
            rd_u32(&self.bytes, base + 4) as usize,  // anchor_len
            rd_u32(&self.bytes, base + 8) as usize,  // loc_idx
            rd_u32(&self.bytes, base + 12) as usize, // loc_n
        )
    }

    fn anchor(&self, i: usize) -> &[u8] {
        let (off, len, _, _) = self.desc(i);
        &self.bytes[off..off + len]
    }

    fn locations(&self, loc_idx: usize, loc_n: usize) -> Vec<Location> {
        let mut out = Vec::with_capacity(loc_n);
        for k in 0..loc_n {
            let base = self.locs_off + (loc_idx + k) * LOC_LEN;
            let file_idx = rd_u32(&self.bytes, base) as usize;
            let offset = rd_u64(&self.bytes, base + 4);
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

    /// Descriptor index of the `k`th wildcard-bearing anchor.
    fn wild(&self, k: usize) -> usize {
        rd_u32(&self.bytes, self.wild_off + k * WILD_LEN) as usize
    }

    /// Add every wildcard anchor satisfying `pred` that `idxs` does not already
    /// hold. Linear, because wildcard anchors do not sort next to the paths
    /// they answer for — but over the wildcard array only, a small fraction of
    /// the corpus. The membership test matters: a query spelling the wildcard
    /// out literally (`...\Services\<name>`) is found by *both* passes, and
    /// would otherwise be reported as two providers documenting one record.
    fn push_wildcard_matches(&self, idxs: &mut Vec<usize>, pred: &dyn Fn(&str) -> bool) {
        for k in 0..self.wild_n {
            let i = self.wild(k);
            if i >= self.entries || idxs.contains(&i) {
                continue;
            }
            if let Ok(anchor) = std::str::from_utf8(self.anchor(i)) {
                if pred(anchor) {
                    idxs.push(i);
                }
            }
        }
    }

    fn collect(&self, idxs: &[usize]) -> Vec<Location> {
        let mut out = Vec::new();
        for &i in idxs {
            let (_, _, loc_idx, loc_n) = self.desc(i);
            out.extend(self.locations(loc_idx, loc_n));
        }
        out
    }

    /// Locations for an exact anchor match, concrete and wildcard alike.
    /// Specificity between the two is applied by `query`, the one funnel every
    /// tier of the cascade passes through.
    pub fn lookup_exact(&self, folded_anchor: &str) -> Vec<Location> {
        let target = folded_anchor.as_bytes();
        let mut idxs = Vec::new();
        let i = self.lower_bound(target);
        if i < self.entries && self.anchor(i) == target {
            idxs.push(i);
        }
        self.push_wildcard_matches(&mut idxs, &|a| scan::is_exact(a, folded_anchor));
        self.collect(&idxs)
    }

    /// Locations for a key query: the key doc plus its directly-attached values.
    /// All concrete anchors sharing the path prefix are contiguous in sorted
    /// order, so we seek the lower bound and walk forward while the prefix
    /// holds; wildcard anchors are gathered separately.
    pub fn lookup_key(&self, folded_path: &str) -> Vec<Location> {
        let path = folded_path.as_bytes();
        let mut idxs = Vec::new();
        let mut i = self.lower_bound(path);
        while i < self.entries && self.anchor(i).starts_with(path) {
            if let Ok(anchor) = std::str::from_utf8(self.anchor(i)) {
                if scan::is_under_key(anchor, folded_path) {
                    idxs.push(i);
                }
            }
            i += 1;
        }
        self.push_wildcard_matches(&mut idxs, &|a| scan::is_under_key(a, folded_path));
        self.collect(&idxs)
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

    #[test]
    #[cfg(unix)]
    fn existing_temporary_symlink_does_not_overwrite_its_target() {
        let temporary = tempfile::tempdir().unwrap();
        let output = temporary.path().join("index");
        let victim = temporary.path().join("unrelated");
        std::fs::write(&victim, b"must survive").unwrap();
        std::os::unix::fs::symlink(&victim, output.with_extension("tmp")).unwrap();
        build(temporary.path(), &output).unwrap();
        assert_eq!(std::fs::read(victim).unwrap(), b"must survive");
        assert!(load(&output).is_some());
    }

    #[test]
    fn truncated_and_corrupt_indexes_are_rejected() {
        let (_temporary, path) = wildcard_built();
        let original = std::fs::read(path).unwrap();
        for length in 0..original.len() {
            assert!(
                decode(original[..length].to_vec()).is_none(),
                "length {length}"
            );
        }
        for offset in [
            12,
            16,
            20,
            24,
            28,
            32,
            36,
            40,
            HEADER_LEN,
            HEADER_LEN + 4,
            HEADER_LEN + 8,
            HEADER_LEN + 12,
        ] {
            let mut invalid = original.clone();
            invalid[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(decode(invalid).is_none(), "offset {offset}");
        }
        let mut invalid = original.clone();
        let locations = rd_u32(&invalid, 20) as usize;
        invalid[locations..locations + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(invalid).is_none());
        let mut invalid = original;
        let wildcards = rd_u32(&invalid, 36) as usize;
        invalid[wildcards..wildcards + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(invalid).is_none());
    }

    #[test]
    fn loaded_snapshot_survives_in_place_file_changes() {
        let (_temporary, path) = built();
        let index = load(&path).unwrap();
        std::fs::write(&path, b"changed").unwrap();
        assert_eq!(index.lookup_exact("machine\\system\\kmes").len(), 1);
    }

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
        assert!(
            text[locs[0].offset as usize..].starts_with("--- machine\\system\\kmes buffercapacity")
        );
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

    fn wildcard_built() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("peinit.regman"),
            "\
--- machine\\system\\services\\<name>
canonical: Machine\\System\\Services\\<name>

One key per service definition.

--- machine\\system\\services\\<name> imagepath
canonical: Machine\\System\\Services\\<name> ImagePath
type: REG_SZ

Absolute path to the service binary.
",
        )
        .unwrap();
        let idx = tmp.path().join(".idx");
        build(tmp.path(), &idx).unwrap();
        (tmp, idx)
    }

    #[test]
    fn wildcard_anchor_is_reachable_through_the_index() {
        let (_t, idxp) = wildcard_built();
        let idx = load(&idxp).unwrap();
        // Binary search alone cannot find this: the anchor does not sort
        // anywhere near the concrete path being asked about.
        let locs = idx.lookup_exact("machine\\system\\services\\sshd imagepath");
        assert_eq!(locs.len(), 1);
        let text = std::fs::read_to_string(&locs[0].file).unwrap();
        assert!(
            text[locs[0].offset as usize..]
                .starts_with("--- machine\\system\\services\\<name> imagepath")
        );
    }

    #[test]
    fn wildcard_key_lookup_returns_doc_and_values() {
        let (_t, idxp) = wildcard_built();
        let idx = load(&idxp).unwrap();
        assert_eq!(idx.lookup_key("machine\\system\\services\\sshd").len(), 2);
    }

    #[test]
    fn literal_wildcard_query_is_not_reported_twice() {
        let (_t, idxp) = wildcard_built();
        let idx = load(&idxp).unwrap();
        // Spelling the placeholder out hits the binary search *and* the
        // wildcard pass; it is still one record, from one provider.
        assert_eq!(
            idx.lookup_exact("machine\\system\\services\\<name> imagepath")
                .len(),
            1
        );
        assert_eq!(idx.lookup_key("machine\\system\\services\\<name>").len(), 2);
    }

    #[test]
    fn concrete_corpus_records_no_wildcards() {
        // The wildcard array stays empty for an ordinary corpus, so the added
        // pass costs nothing where nothing uses it.
        let (_t, idxp) = built();
        let idx = load(&idxp).unwrap();
        assert_eq!(idx.wild_n, 0);
    }

    #[test]
    fn stale_index_version_is_rejected_not_misread() {
        // A v1 index has no wildcard array; reading it as v2 would walk off
        // into the anchor blob. The version guard must reject it outright so
        // the cascade falls back to a scan.
        let (_t, idxp) = built();
        let mut bytes = std::fs::read(&idxp).unwrap();
        bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        std::fs::write(&idxp, &bytes).unwrap();
        assert!(load(&idxp).is_none());
    }
}
