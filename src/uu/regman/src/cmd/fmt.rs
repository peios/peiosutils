// `regman fmt <file>...` — bake the folded anchor onto each fence line from the
// record's `canonical` field (design §4.5). Authors write only `canonical`;
// fmt keeps the search anchor in sync.

use clap::ArgMatches;

use crate::error::{Error, Result};
use crate::fragment::{self, ParseIssue};

pub fn run(matches: &ArgMatches) -> Result<()> {
    let files: Vec<&String> = matches
        .get_many::<String>("files")
        .map(Iterator::collect)
        .unwrap_or_default();

    let mut had_issue = false;
    for file in files {
        let text = std::fs::read_to_string(file)?;
        let (baked, issues) = bake(&text);
        for issue in &issues {
            eprintln!("{file}: line {}: {}", issue.line, issue.message);
            had_issue = true;
        }
        if baked != text {
            std::fs::write(file, &baked)?;
            println!("{file}: anchors updated");
        }
    }

    if had_issue {
        return Err(Error::Fragment(
            "fmt: some records have no `canonical:` and were skipped".to_string(),
        ));
    }
    Ok(())
}

/// Rewrite every fence line to `--- <fold(canonical)>`. Edits are applied from
/// the end of the file so earlier byte offsets stay valid.
pub fn bake(text: &str) -> (String, Vec<ParseIssue>) {
    let (records, issues) = fragment::parse(text);

    let mut edits: Vec<(usize, usize, String)> = records
        .iter()
        .map(|r| {
            let line_end = text[r.fence_offset..]
                .find('\n')
                .map_or(text.len(), |i| r.fence_offset + i);
            (r.fence_offset, line_end, format!("--- {}", r.expected_anchor()))
        })
        .collect();
    edits.sort_by(|a, b| b.0.cmp(&a.0));

    let mut out = text.to_string();
    for (start, end, replacement) in edits {
        out.replace_range(start..end, &replacement);
    }
    (out, issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bakes_wrong_anchor_from_canonical() {
        let text = "\
--- totally wrong
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD

body
";
        let (out, issues) = bake(text);
        assert!(issues.is_empty());
        assert!(out.starts_with("--- machine\\system\\kmes buffercapacity\n"));
        assert!(out.contains("canonical: Machine\\System\\KMES BufferCapacity"));
        assert!(out.contains("type: REG_QWORD"));
    }

    #[test]
    fn idempotent_on_correct_input() {
        let text = "\
--- machine\\system\\kmes buffercapacity
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD

body
";
        let (out, _) = bake(text);
        assert_eq!(out, text);
    }

    #[test]
    fn bakes_multiple_records() {
        let text = "\
--- x
canonical: Machine\\System\\KMES

key body

--- y
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD

value body
";
        let (out, _) = bake(text);
        assert!(out.contains("--- machine\\system\\kmes\n"));
        assert!(out.contains("--- machine\\system\\kmes buffercapacity\n"));
    }
}
