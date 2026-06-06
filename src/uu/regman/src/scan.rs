// The authoritative lookup: scan the on-disk corpus.
//
// This path uses no index and is always correct (design §5). The query anchor
// is folded once, then each fragment is rejected with a raw byte substring
// pre-scan (`memmem`) before being parsed — a file that doesn't contain the
// anchor bytes contiguously cannot hold a matching record, so it's skipped at
// memory-bandwidth speed. Survivors are parsed and confirmed on the parsed
// anchor (rejecting incidental matches in body prose).

use std::path::{Path, PathBuf};

use memchr::memmem;

use crate::corpus;
use crate::error::Result;
use crate::fold::fold;
use crate::fragment::{self, Record};

/// One matched record together with where it came from.
#[derive(Debug, Clone)]
pub struct Hit {
    pub provider: String,
    pub file: PathBuf,
    pub record: Record,
}

/// Does `anchor` denote the exact `(path, value)` named by `folded`?
pub fn is_exact(anchor: &str, folded: &str) -> bool {
    anchor == folded
}

/// Does `anchor` belong to the key `folded_path` — i.e. the key doc itself, or
/// one of its directly-attached values? The trailing-space test prevents a
/// sibling key (`...\kmesfoo`) from matching a prefix of `...\kmes`.
pub fn is_under_key(anchor: &str, folded_path: &str) -> bool {
    anchor == folded_path
        || (anchor.len() > folded_path.len()
            && anchor.starts_with(folded_path)
            && anchor.as_bytes()[folded_path.len()] == b' ')
}

/// Exact match across the whole corpus.
pub fn corpus_exact(dir: &Path, folded_anchor: &str) -> Result<Vec<Hit>> {
    let mut hits = Vec::new();
    for file in corpus::fragments(dir)? {
        hits.extend(file_exact(&file, folded_anchor)?);
    }
    sort_by_provider(&mut hits);
    Ok(hits)
}

/// Key match across the whole corpus.
pub fn corpus_key(dir: &Path, folded_path: &str) -> Result<Vec<Hit>> {
    let mut hits = Vec::new();
    for file in corpus::fragments(dir)? {
        hits.extend(file_key(&file, folded_path)?);
    }
    sort_by_provider(&mut hits);
    Ok(hits)
}

/// Exact match within a single fragment file.
pub fn file_exact(file: &Path, folded_anchor: &str) -> Result<Vec<Hit>> {
    scan_file(file, folded_anchor.as_bytes(), &|r| is_exact(&r.anchor, folded_anchor))
}

/// Key match within a single fragment file.
pub fn file_key(file: &Path, folded_path: &str) -> Result<Vec<Hit>> {
    scan_file(file, folded_path.as_bytes(), &|r| is_under_key(&r.anchor, folded_path))
}

/// `man -k` / apropos: every record whose name *and* summary together contain
/// all the (case-folded) terms. Searches the canonical name + one-line summary
/// — the short-description analog — not full bodies. Results are sorted by
/// canonical name. This does not use the index (it's a content search, not an
/// anchor lookup, design §8).
pub fn apropos(dir: &Path, terms: &[String]) -> Result<Vec<Hit>> {
    let folded: Vec<String> = terms.iter().map(|t| fold(t)).collect();
    let mut hits = Vec::new();
    for file in corpus::fragments(dir)? {
        let text = std::fs::read_to_string(&file)?;
        let provider = corpus::provider_of(&file);
        let (records, _) = fragment::parse(&text);
        for record in records {
            let haystack = fold(&format!("{} {}", record.canonical, record.summary()));
            if folded.iter().all(|t| haystack.contains(t.as_str())) {
                hits.push(Hit {
                    provider: provider.clone(),
                    file: file.clone(),
                    record,
                });
            }
        }
    }
    hits.sort_by(|a, b| {
        fold(&a.record.canonical)
            .cmp(&fold(&b.record.canonical))
            .then(a.provider.cmp(&b.provider))
    });
    Ok(hits)
}

fn scan_file(file: &Path, needle: &[u8], pred: &dyn Fn(&Record) -> bool) -> Result<Vec<Hit>> {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        // A fragment that vanished (e.g. renamed since the index was built)
        // simply contributes nothing — the cascade falls through.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    // Cheap rejection: if the anchor bytes aren't present at all, skip parsing.
    if memmem::find(text.as_bytes(), needle).is_none() {
        return Ok(Vec::new());
    }
    let provider = corpus::provider_of(file);
    let (records, _issues) = fragment::parse(&text);
    Ok(records
        .into_iter()
        .filter(|r| pred(r))
        .map(|record| Hit {
            provider: provider.clone(),
            file: file.to_path_buf(),
            record,
        })
        .collect())
}

fn sort_by_provider(hits: &mut [Hit]) {
    hits.sort_by(|a, b| a.provider.cmp(&b.provider).then(a.record.anchor.cmp(&b.record.anchor)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_corpus() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("kmes.regman"),
            "\
--- machine\\system\\kmes
canonical: Machine\\System\\KMES

KMES config subtree.

--- machine\\system\\kmes buffercapacity
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD
default: 4194304
applies: live

Per-CPU ring buffer capacity.

--- machine\\system\\kmesfoo
canonical: Machine\\System\\KMESFoo

A sibling key that must NOT match a kmes prefix query.
",
        )
        .unwrap();
        tmp
    }

    #[test]
    fn exact_finds_one_value() {
        let tmp = write_corpus();
        let hits = corpus_exact(tmp.path(), "machine\\system\\kmes buffercapacity").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].provider, "kmes");
        assert_eq!(hits[0].record.canonical, "Machine\\System\\KMES BufferCapacity");
    }

    #[test]
    fn exact_miss_is_empty() {
        let tmp = write_corpus();
        assert!(corpus_exact(tmp.path(), "machine\\system\\kmes nope").unwrap().is_empty());
    }

    #[test]
    fn key_returns_key_doc_and_its_values_only() {
        let tmp = write_corpus();
        let hits = corpus_key(tmp.path(), "machine\\system\\kmes").unwrap();
        let anchors: Vec<_> = hits.iter().map(|h| h.record.anchor.as_str()).collect();
        assert_eq!(
            anchors,
            vec!["machine\\system\\kmes", "machine\\system\\kmes buffercapacity"]
        );
        // The sibling kmesfoo key is excluded.
        assert!(!anchors.iter().any(|a| a.contains("kmesfoo")));
    }

    #[test]
    fn apropos_matches_name_and_summary_case_insensitively() {
        let tmp = write_corpus();
        // "capacity" appears in BufferCapacity's name and summary.
        let hits = apropos(tmp.path(), &["CAPACITY".to_string()]).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record.canonical, "Machine\\System\\KMES BufferCapacity");
    }

    #[test]
    fn apropos_requires_all_terms() {
        let tmp = write_corpus();
        // "buffer" matches, but "nonsense" does not — AND semantics ⇒ no hit.
        assert!(apropos(tmp.path(), &["buffer".into(), "nonsense".into()]).unwrap().is_empty());
        // Both present ⇒ hit.
        let hits = apropos(tmp.path(), &["per-cpu".into(), "ring".into()]).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn under_key_predicate_rejects_sibling_prefix() {
        assert!(is_under_key("machine\\system\\kmes", "machine\\system\\kmes"));
        assert!(is_under_key("machine\\system\\kmes buffercapacity", "machine\\system\\kmes"));
        assert!(!is_under_key("machine\\system\\kmesfoo", "machine\\system\\kmes"));
    }
}
