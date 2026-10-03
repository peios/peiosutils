// Fragment data model + parser.
//
// A fragment file (`/usr/share/regman/<pkg>.regman`) is a sequence of records
// in the fenced multi-document format (design §4.2):
//
//     --- <folded anchor>
//     canonical: <Original-Case Path[ Value]>
//     type: REG_DWORD
//     ...
//                                  <- blank line starts the body
//     Markdown body until the next "--- " fence.
//
// The fence line both delimits the record and carries the case-folded search
// anchor; `canonical` carries the display form. A record with any value-only
// field (type/default/valid/applies) is a *value* doc; otherwise it is a
// *key* doc — the absence of those fields is how we tell them apart (§4.4),
// not by parsing the anchor (which we cannot reliably split, since both key
// paths and value names may contain spaces).

use crate::fold::fold;

/// A documented registry key or value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Byte offset of the record's fence line within its source file. Used by
    /// the index to point straight at a record.
    pub fence_offset: usize,
    /// The case-folded `path[ value]` anchor from the fence line.
    pub anchor: String,
    /// Original-case `path[ value]` for display (the `canonical:` field).
    pub canonical: String,
    /// Low-level registry type tag (`REG_DWORD`, ...). Value docs only.
    pub type_: Option<String>,
    /// Documented default.
    pub default: Option<String>,
    /// Valid range / enum / constraints, human-readable.
    pub valid: Option<String>,
    /// When a change takes effect: `live` | `restart` | `reboot`.
    pub applies: Option<String>,
    /// Present ⇒ being retired; the value is the replacement / note.
    pub deprecated: Option<String>,
    /// Markdown body (leading/trailing blank lines trimmed).
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Key,
    Value,
}

impl Record {
    /// A record is a value doc if it carries any value-only field.
    pub fn kind(&self) -> Kind {
        if self.type_.is_some()
            || self.default.is_some()
            || self.valid.is_some()
            || self.applies.is_some()
        {
            Kind::Value
        } else {
            Kind::Key
        }
    }

    /// The first non-empty line of the body — the one-line summary used in the
    /// key-level Values index (design §9.2; the docstring/git-subject rule).
    pub fn summary(&self) -> &str {
        self.body
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("")
    }

    /// Display value-name for the Values index, given the canonical key path it
    /// hangs under. Strips the known key prefix (robust even if the value name
    /// contains spaces); falls back to the last-space split when the prefix
    /// does not line up.
    pub fn value_name(&self, key_canonical: &str) -> &str {
        let prefix_len = key_canonical.len() + 1; // path + separating space
        if self.canonical.len() > prefix_len
            && self.canonical.is_char_boundary(prefix_len)
            && fold(&self.canonical[..key_canonical.len()]) == fold(key_canonical)
            && self.canonical.as_bytes()[key_canonical.len()] == b' '
        {
            return &self.canonical[prefix_len..];
        }
        match self.canonical.rsplit_once(' ') {
            Some((_, v)) => v,
            None => &self.canonical,
        }
    }

    /// Expected fence anchor derived from `canonical` (design §4.5): the anchor
    /// is simply the fold of the canonical string. `fmt` bakes this; `lint`
    /// checks it.
    pub fn expected_anchor(&self) -> String {
        fold(&self.canonical)
    }
}

/// A problem found while parsing a fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseIssue {
    /// 1-based line number of the fence that opened the offending record.
    pub line: usize,
    pub message: String,
}

/// Parse a fragment's text into records. Content before the first fence is
/// ignored. Header lines are `key: value`; unknown keys are ignored for
/// forward compatibility (`lint` is stricter). A record is rejected only if it
/// is structurally impossible (e.g. missing `canonical`); such issues are
/// returned alongside the records that did parse.
pub fn parse(text: &str) -> (Vec<Record>, Vec<ParseIssue>) {
    let mut records = Vec::new();
    let mut issues = Vec::new();

    // Collect (fence_offset, anchor, line_no, body_start_offset) for each fence,
    // walking the text line by line while tracking byte offsets.
    let mut fences: Vec<Fence> = Vec::new();
    let mut offset = 0usize;
    for (line_no, line) in line_spans(text) {
        if let Some(anchor) = fence_anchor(line) {
            fences.push(Fence {
                offset,
                line_no,
                anchor: anchor.to_string(),
            });
        }
        offset += line.len() + 1; // +1 for the '\n' (or virtual newline at EOF)
    }

    for (i, fence) in fences.iter().enumerate() {
        let record_start = fence.offset;
        let record_end = fences
            .get(i + 1).map_or_else(|| text.len(), |f| f.offset);
        let block = &text[record_start..record_end];

        // The first line of `block` is the fence itself; skip it.
        let after_fence = block.split_once('\n').map_or("", |x| x.1);
        let (header_text, body) = split_header_body(after_fence);
        let header = parse_header(header_text);

        let Some(canonical) = header.canonical else {
            issues.push(ParseIssue {
                line: fence.line_no,
                message: "record has no `canonical:` field".to_string(),
            });
            continue;
        };

        records.push(Record {
            fence_offset: fence.offset,
            anchor: fence.anchor.clone(),
            canonical,
            type_: header.type_,
            default: header.default,
            valid: header.valid,
            applies: header.applies,
            deprecated: header.deprecated,
            body: body.trim_matches('\n').to_string(),
        });
    }

    (records, issues)
}

struct Fence {
    offset: usize,
    line_no: usize,
    anchor: String,
}

/// If `line` is a fence, return its anchor (the trimmed text after `--- `).
/// A bare `---` (a Markdown thematic break) is not a fence.
pub fn fence_anchor(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("---")?;
    let rest = rest.strip_prefix(' ')?;
    let anchor = rest.trim();
    if anchor.is_empty() { None } else { Some(anchor) }
}

/// Iterate (1-based line number, line text without trailing '\n').
fn line_spans(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split('\n')
        .enumerate()
        .map(|(i, l)| (i + 1, l.strip_suffix('\r').unwrap_or(l)))
}

/// Split the post-fence block into (header, body) at the first blank line.
fn split_header_body(after_fence: &str) -> (&str, &str) {
    let mut idx = 0usize;
    for line in after_fence.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
        if trimmed.is_empty() {
            let body_start = idx + line.len();
            return (&after_fence[..idx], &after_fence[body_start..]);
        }
        idx += line.len();
    }
    (after_fence, "")
}

#[derive(Default)]
struct Header {
    canonical: Option<String>,
    type_: Option<String>,
    default: Option<String>,
    valid: Option<String>,
    applies: Option<String>,
    deprecated: Option<String>,
}

fn parse_header(text: &str) -> Header {
    let mut h = Header::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().to_string();
        match key.trim().to_ascii_lowercase().as_str() {
            "canonical" => h.canonical = Some(value),
            "type" => h.type_ = Some(value),
            "default" => h.default = Some(value),
            "valid" => h.valid = Some(value),
            "applies" => h.applies = Some(value),
            "deprecated" => h.deprecated = Some(value),
            _ => {} // unknown field: ignored for forward-compat
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_DOC: &str = "\
--- machine\\system\\kmes
canonical: Machine\\System\\KMES

KMES reads its operational parameters from the values under this key.
Second paragraph.
";

    const VALUE_DOC: &str = "\
--- machine\\system\\kmes buffercapacity
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD
default: 4194304
valid: 65536-268435456 bytes, power of two
applies: live

Per-CPU ring buffer capacity, in bytes.
";

    #[test]
    fn parses_key_doc() {
        let (recs, issues) = parse(KEY_DOC);
        assert!(issues.is_empty());
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r.anchor, "machine\\system\\kmes");
        assert_eq!(r.canonical, "Machine\\System\\KMES");
        assert_eq!(r.kind(), Kind::Key);
        assert_eq!(r.fence_offset, 0);
        assert!(r.body.starts_with("KMES reads"));
    }

    #[test]
    fn parses_value_doc_fields() {
        let (recs, _) = parse(VALUE_DOC);
        let r = &recs[0];
        assert_eq!(r.kind(), Kind::Value);
        assert_eq!(r.type_.as_deref(), Some("REG_QWORD"));
        assert_eq!(r.default.as_deref(), Some("4194304"));
        assert_eq!(r.applies.as_deref(), Some("live"));
        assert_eq!(r.summary(), "Per-CPU ring buffer capacity, in bytes.");
    }

    #[test]
    fn parses_multiple_records_with_offsets() {
        let text = format!("{KEY_DOC}{VALUE_DOC}");
        let (recs, issues) = parse(&text);
        assert!(issues.is_empty());
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].fence_offset, 0);
        assert_eq!(recs[1].fence_offset, KEY_DOC.len());
        // Round-trip: the byte offset really points at the second fence.
        assert!(text[recs[1].fence_offset..].starts_with("--- machine\\system\\kmes buffercapacity"));
    }

    #[test]
    fn missing_canonical_is_an_issue_not_a_record() {
        let text = "--- machine\\system\\kmes\ntype: REG_DWORD\n\nbody\n";
        let (recs, issues) = parse(text);
        assert!(recs.is_empty());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].line, 1);
    }

    #[test]
    fn bare_dashes_are_not_a_fence() {
        assert_eq!(fence_anchor("---"), None);
        assert_eq!(fence_anchor("--- machine\\x"), Some("machine\\x"));
        assert_eq!(fence_anchor("text --- more"), None);
    }

    #[test]
    fn expected_anchor_is_fold_of_canonical() {
        let (recs, _) = parse(VALUE_DOC);
        assert_eq!(recs[0].expected_anchor(), recs[0].anchor);
    }

    #[test]
    fn value_name_strips_key_prefix() {
        let (recs, _) = parse(VALUE_DOC);
        assert_eq!(recs[0].value_name("Machine\\System\\KMES"), "BufferCapacity");
    }

    #[test]
    fn value_name_handles_spaces_in_value() {
        let text = "\
--- machine\\system\\x safe mode restarts
canonical: Machine\\System\\X Safe Mode Restarts
type: REG_DWORD

body
";
        let (recs, _) = parse(text);
        assert_eq!(recs[0].value_name("Machine\\System\\X"), "Safe Mode Restarts");
    }

    #[test]
    fn unknown_header_fields_ignored() {
        let text = "--- a\\b\ncanonical: A\\B\nbogus: whatever\n\nbody\n";
        let (recs, issues) = parse(text);
        assert!(issues.is_empty());
        assert_eq!(recs[0].kind(), Kind::Key);
    }
}
