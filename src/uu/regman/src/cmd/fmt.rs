// `regman fmt <file>...` — bake the folded anchor onto each fence line from the
// record's `canonical` field (design §4.5). Authors write only `canonical`;
// fmt keeps the search anchor in sync.

use std::cmp::Reverse;

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
            let line_end = text[r.fence_offset..].find('\n').map_or(text.len(), |i| {
                let end = r.fence_offset + i;
                // Replace the fence content, preserving its LF or CRLF.
                if i > 0 && text.as_bytes()[end - 1] == b'\r' {
                    end - 1
                } else {
                    end
                }
            });
            (
                r.fence_offset,
                line_end,
                format!("--- {}", r.expected_anchor()),
            )
        })
        .collect();
    edits.sort_by_key(|edit| Reverse(edit.0));

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

    #[test]
    fn preserves_mixed_endings_and_utf8_bodies() {
        let text = "Preamble: café\r\n--- x\r\ncanonical: Machine\\System\\Café\r\n\r\nRésumé: λ🙂\r\n\r\n--- y\ncanonical: Machine\\System\\Other\n\nBody: unchanged\n--- z\r\ncanonical: Machine\\System\\Last";
        let expected = text
            .replace("--- x\r\n", "--- machine\\system\\café\r\n")
            .replace("--- y\n", "--- machine\\system\\other\n")
            .replace("--- z\r\n", "--- machine\\system\\last\r\n");
        let (baked, issues) = bake(text);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(baked, expected);
        let (again, issues) = bake(&baked);
        assert!(issues.is_empty());
        assert_eq!(again, baked);
    }

    #[test]
    fn correct_crlf_input_is_unchanged() {
        let text = "--- a\\b\r\ncanonical: A\\B\r\n\r\nBody.\r\n";
        let (baked, issues) = bake(text);
        assert!(issues.is_empty());
        assert_eq!(baked, text);
    }
}
