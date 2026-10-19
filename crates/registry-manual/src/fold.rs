// Case folding for registry anchors.
//
// regman must fold exactly as LCS does (PSD-005 §1) so the two agree on what
// "the same key" means. This is Unicode Simple Case Folding pinned to Unicode
// 16.0 (CaseFolding.txt statuses C and S) — the same algorithm and table as
// pkm/crates/lcs-core/src/casefold.rs, mirrored here (see casefold_table.rs)
// rather than taking a dependency on the kernel registry core. Folding lives
// only in this module.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CaseFoldRange {
    start: u32,
    end: u32,
    mapped_start: u32,
}

include!("casefold_table.rs");

/// Apply Unicode Simple Case Folding to one scalar value.
fn fold_char(value: char) -> char {
    let codepoint = value as u32;
    fold_from_ranges(codepoint)
        .or_else(|| fold_from_pairs(codepoint))
        .unwrap_or(value)
}

/// Fold a string to its canonical case-insensitive form. Idempotent, and equal
/// for any two strings the registry considers case-insensitively equal.
pub fn fold(s: &str) -> String {
    s.chars().map(fold_char).collect()
}

fn fold_from_ranges(codepoint: u32) -> Option<char> {
    let mut low = 0usize;
    let mut high = CASE_FOLD_RANGES.len();
    while low < high {
        let mid = low.midpoint(high);
        let range = CASE_FOLD_RANGES[mid];
        if codepoint < range.start {
            high = mid;
        } else if codepoint > range.end {
            low = mid + 1;
        } else {
            let mapped = range.mapped_start + (codepoint - range.start);
            return Some(char::from_u32(mapped).expect("casefold range target is scalar"));
        }
    }
    None
}

fn fold_from_pairs(codepoint: u32) -> Option<char> {
    use std::cmp::Ordering;
    let mut low = 0usize;
    let mut high = CASE_FOLD_PAIRS.len();
    while low < high {
        let mid = low.midpoint(high);
        let (source, target) = CASE_FOLD_PAIRS[mid];
        match codepoint.cmp(&source) {
            Ordering::Less => high = mid,
            Ordering::Greater => low = mid + 1,
            Ordering::Equal => {
                return Some(char::from_u32(target).expect("casefold pair target is scalar"));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_lowercases() {
        assert_eq!(fold(r"Machine\System\KMES"), r"machine\system\kmes");
        assert_eq!(fold("BufferCapacity"), "buffercapacity");
    }

    #[test]
    fn idempotent() {
        let once = fold(r"Machine\System\KMES BufferCapacity");
        assert_eq!(fold(&once), once);
    }

    #[test]
    fn preserves_separators_and_spaces() {
        assert_eq!(fold(r"A\B C"), r"a\b c");
    }

    #[test]
    fn case_insensitive_equality() {
        assert_eq!(fold("KMES"), fold("kmes"));
        assert_eq!(fold("MaxEventSize"), fold("MAXEVENTSIZE"));
    }

    #[test]
    fn folds_non_ascii_per_unicode_scf() {
        // Greek capital sigma → small sigma (range table).
        assert_eq!(fold("Σ"), "σ");
        // Cyrillic capital A → small a.
        assert_eq!(fold("А"), "а");
        // Micro sign µ → Greek small mu (pairs table), matching LCS.
        assert_eq!(fold("µ"), "μ");
        assert_eq!(fold("Σ"), fold("σ"));
    }

    #[test]
    fn diverges_from_naive_lowercasing_where_scf_does() {
        // Kelvin sign (U+212A) simple-case-folds to ASCII 'k'; this is exactly
        // the kind of case the SCF table gets right and a quick approximation
        // might not. Confirms we're using the real table.
        assert_eq!(fold("\u{212A}"), "k");
    }
}
