// Resolving a query: the soft-failure cascade (design §7.3).
//
// A lookup with an index walks three tiers, each validating and degrading
// rather than trusting blindly, so a stale index costs speed, never
// correctness:
//
//   1. exact pointer — seek to the recorded offset, confirm the anchor matches
//   2. whole file    — scan the file(s) the index named
//   3. global        — scan the whole corpus (the authoritative backstop)
//
// A true index miss (no entry for the anchor) goes straight to tier 3. The
// cascade self-heals churn (moved/renamed/deleted records); it does NOT
// discover a newly-added provider for an already-indexed path — that rests on
// reindex (the accepted trust contract).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::corpus;
use crate::error::Result;
use crate::fragment::{self, Record};
use crate::index::{self, Location};
use crate::scan::{self, Hit};

/// Resolve an exact `(path, value)` query.
pub fn resolve_exact(dir: &Path, index_path: &Path, folded_anchor: &str) -> Result<Vec<Hit>> {
    cascade(
        dir,
        index_path,
        |idx| idx.lookup_exact(folded_anchor),
        |file| scan::file_exact(file, folded_anchor),
        |d| scan::corpus_exact(d, folded_anchor),
        &|r| scan::is_exact(&r.anchor, folded_anchor),
    )
}

/// Resolve a key query (key doc + its directly-attached values).
pub fn resolve_key(dir: &Path, index_path: &Path, folded_path: &str) -> Result<Vec<Hit>> {
    cascade(
        dir,
        index_path,
        |idx| idx.lookup_key(folded_path),
        |file| scan::file_key(file, folded_path),
        |d| scan::corpus_key(d, folded_path),
        &|r| scan::is_under_key(&r.anchor, folded_path),
    )
}

fn cascade(
    dir: &Path,
    index_path: &Path,
    lookup: impl Fn(&index::Index) -> Vec<Location>,
    file_scan: impl Fn(&Path) -> Result<Vec<Hit>>,
    corpus_scan: impl Fn(&Path) -> Result<Vec<Hit>>,
    pred: &dyn Fn(&Record) -> bool,
) -> Result<Vec<Hit>> {
    if let Some(idx) = index::load(index_path) {
        let locs = lookup(&idx);
        if !locs.is_empty() {
            // Tier 1: validate each recorded pointer.
            let mut hits = Vec::new();
            let mut stale = false;
            for loc in &locs {
                match hit_at(loc, pred) {
                    Some(h) => hits.push(h),
                    None => stale = true,
                }
            }
            if !stale && !hits.is_empty() {
                sort_hits(&mut hits);
                return Ok(hits);
            }
            // Tier 2: rescan the file(s) the index named.
            let mut hits = Vec::new();
            for file in distinct_files(&locs) {
                hits.extend(file_scan(&file)?);
            }
            if !hits.is_empty() {
                sort_hits(&mut hits);
                return Ok(hits);
            }
            // Otherwise fall through to the global scan.
        }
    }
    // Tier 3: authoritative global scan.
    corpus_scan(dir)
}

/// Read the record at a recorded location and confirm it still matches. A
/// missing file or a non-matching record means the pointer is stale.
fn hit_at(loc: &Location, pred: &dyn Fn(&Record) -> bool) -> Option<Hit> {
    let text = std::fs::read_to_string(&loc.file).ok()?;
    let (records, _) = fragment::parse(&text);
    let record = records
        .into_iter()
        .find(|r| r.fence_offset as u64 == loc.offset && pred(r))?;
    Some(Hit {
        provider: corpus::provider_of(&loc.file),
        file: loc.file.clone(),
        record,
    })
}

fn distinct_files(locs: &[Location]) -> Vec<PathBuf> {
    let set: BTreeSet<PathBuf> = locs.iter().map(|l| l.file.clone()).collect();
    set.into_iter().collect()
}

fn sort_hits(hits: &mut [Hit]) {
    hits.sort_by(|a, b| {
        a.provider
            .cmp(&b.provider)
            .then(a.record.anchor.cmp(&b.record.anchor))
    });
}

/// Convenience: resolve against the environment-configured corpus/index.
pub fn resolve_exact_default(folded_anchor: &str) -> Result<Vec<Hit>> {
    resolve_exact(&corpus::dir(), &corpus::index_path(), folded_anchor)
}
/// Convenience: resolve a key query against the environment-configured paths.
pub fn resolve_key_default(folded_path: &str) -> Result<Vec<Hit>> {
    resolve_key(&corpus::dir(), &corpus::index_path(), folded_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAG: &str = "\
--- machine\\system\\kmes
canonical: Machine\\System\\KMES

KMES subtree.

--- machine\\system\\kmes buffercapacity
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD

Ring buffer capacity.
";

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let frag = tmp.path().join("kmes.regman");
        std::fs::write(&frag, FRAG).unwrap();
        let idxp = tmp.path().join(".idx");
        index::build(tmp.path(), &idxp).unwrap();
        (tmp, idxp, frag)
    }

    #[test]
    fn tier1_exact_hit() {
        let (tmp, idxp, _) = setup();
        let hits = resolve_exact(tmp.path(), &idxp, "machine\\system\\kmes buffercapacity").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record.canonical, "Machine\\System\\KMES BufferCapacity");
    }

    #[test]
    fn no_index_falls_to_global_scan() {
        let (tmp, _idxp, _) = setup();
        let missing = tmp.path().join("does-not-exist.idx");
        let hits = resolve_exact(tmp.path(), &missing, "machine\\system\\kmes buffercapacity").unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn stale_pointer_degrades_to_whole_file_then_finds_it() {
        let (tmp, idxp, frag) = setup();
        // Prepend a record so every offset in the index shifts — the recorded
        // pointers are now stale, but tier 2 (whole-file scan) must still find it.
        let shifted = format!(
            "--- machine\\system\\kmes prelude\ncanonical: Machine\\System\\KMES Prelude\ntype: REG_DWORD\n\nAdded later.\n\n{FRAG}"
        );
        std::fs::write(&frag, shifted).unwrap();
        let hits = resolve_exact(tmp.path(), &idxp, "machine\\system\\kmes buffercapacity").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record.canonical, "Machine\\System\\KMES BufferCapacity");
    }

    #[test]
    fn deleted_file_degrades_to_global_scan() {
        let (tmp, idxp, frag) = setup();
        // Move the docs to a different file: the indexed file is gone, but the
        // record still exists elsewhere in the corpus.
        std::fs::rename(&frag, tmp.path().join("renamed.regman")).unwrap();
        let hits = resolve_exact(tmp.path(), &idxp, "machine\\system\\kmes buffercapacity").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].provider, "renamed");
    }

    #[test]
    fn key_query_returns_doc_and_value() {
        let (tmp, idxp, _) = setup();
        let hits = resolve_key(tmp.path(), &idxp, "machine\\system\\kmes").unwrap();
        assert_eq!(hits.len(), 2);
    }
}
