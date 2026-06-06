// `regman lint <file>...` — verify fragment structure and anchors.

use clap::ArgMatches;

use crate::error::{Error, Result};
use crate::fragment;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let files: Vec<&String> = matches
        .get_many::<String>("files")
        .map(Iterator::collect)
        .unwrap_or_default();

    let mut total = 0usize;
    for file in files {
        let text = std::fs::read_to_string(file)?;
        for problem in lint_text(&text) {
            eprintln!("{file}: {problem}");
            total += 1;
        }
    }

    if total > 0 {
        return Err(Error::Fragment(format!("lint: {total} problem(s) found")));
    }
    Ok(())
}

/// Structural problems in a fragment: parse issues (e.g. a record with no
/// `canonical:`) and any fence anchor that disagrees with `fold(canonical)`.
pub fn lint_text(text: &str) -> Vec<String> {
    let (records, issues) = fragment::parse(text);
    let mut out: Vec<String> = issues
        .iter()
        .map(|i| format!("line {}: {}", i.line, i.message))
        .collect();

    for r in &records {
        let expected = r.expected_anchor();
        if r.anchor != expected {
            out.push(format!(
                "anchor for `{}`: fence has `{}`, expected `{expected}` (run `regman fmt`)",
                r.canonical, r.anchor
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_fragment_has_no_problems() {
        let text = "\
--- machine\\system\\kmes buffercapacity
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD

body
";
        assert!(lint_text(text).is_empty());
    }

    #[test]
    fn flags_anchor_mismatch() {
        let text = "\
--- wrong anchor
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD

body
";
        let problems = lint_text(text);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("anchor for"));
    }

    #[test]
    fn flags_missing_canonical() {
        let text = "--- machine\\system\\kmes\ntype: REG_DWORD\n\nbody\n";
        let problems = lint_text(text);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("no `canonical:`"));
    }
}
