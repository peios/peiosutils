// Knob-card rendering (design §9).
//
// A registry item is a knob, not a command, so regman does not ape `man`'s
// freeform sections. It renders a consistent card: identity line → aligned
// field block (only the fields present) → prose body. The key-level view
// doubles as an index: the key body followed by a `Values` list with a one-line
// summary per value (the first line of each value's body).
//
// These functions are pure and take an explicit `width` and `Style` so output
// is deterministic and testable; tty concerns (paging) live in the pager.

use crate::fragment::{Kind, Record};
use crate::markdown::{self, Style};
use crate::scan::Hit;

/// Render the result of an exact `(path, value)` query.
pub fn exact(hits: &[Hit], width: usize, style: Style) -> String {
    if hits.len() == 1 {
        value_card(&hits[0].record, &hits[0].provider, width, style)
    } else {
        let mut s = format!("documented by {} packages:\n", hits.len());
        for h in hits {
            s.push('\n');
            s.push_str(&value_card(&h.record, &h.provider, width, style));
        }
        s
    }
}

/// Render a key-level query: key body (if documented) + the Values index.
/// `query_canonical` is the path the user asked about, used to derive value
/// names when no key-level doc supplies the canonical key path.
pub fn key(hits: &[Hit], query_canonical: &str, width: usize, style: Style) -> String {
    let key_docs: Vec<&Hit> = hits.iter().filter(|h| h.record.kind() == Kind::Key).collect();
    let value_docs: Vec<&Hit> = hits.iter().filter(|h| h.record.kind() == Kind::Value).collect();

    let key_canonical = key_docs
        .first()
        .map_or_else(|| query_canonical.to_string(), |h| h.record.canonical.clone());

    let mut s = String::new();

    if key_docs.len() > 1 {
        s.push_str(&format!("documented by {} packages:\n\n", key_docs.len()));
    }
    if let Some(h) = key_docs.first() {
        s.push_str(&identity_line(&h.record.canonical, &h.provider, width, style));
        s.push('\n');
        if let Some(dep) = &h.record.deprecated {
            s.push_str(&format!("\n{}\n", style.warn(&format!("DEPRECATED: {dep}"))));
        }
        let body = markdown::render(&h.record.body, width, style);
        if !body.is_empty() {
            s.push('\n');
            s.push_str(&body);
            s.push('\n');
        }
    } else {
        // No key-level doc: still head the listing with the path.
        s.push_str(&style.bold(&key_canonical));
        s.push('\n');
    }

    if !value_docs.is_empty() {
        s.push_str(&format!("\n{}\n", style.bold("Values")));
        let names: Vec<&str> = value_docs
            .iter()
            .map(|h| h.record.value_name(&key_canonical))
            .collect();
        let w = names.iter().map(|n| n.len()).max().unwrap_or(0);
        // Keep each value to a single line: truncate the summary to the space
        // left after the name column.
        let avail = width.saturating_sub(w + 4);
        for (h, name) in value_docs.iter().zip(&names) {
            let summary = truncate(&markdown::strip_inline(h.record.summary()), avail);
            let padded = format!("{name:<w$}");
            s.push_str(&format!("  {}  {summary}\n", style.bold(&padded)));
        }
    }

    s
}

/// Render apropos (`-k`) results: one line per match, `name  summary`, the
/// summary truncated to fit. Like `man -k`, but with the registry name in place
/// of `name(section)`.
pub fn apropos(hits: &[Hit], width: usize, style: Style) -> String {
    let mut s = String::new();
    for h in hits {
        let canon = &h.record.canonical;
        let summary = markdown::strip_inline(h.record.summary());
        let avail = width.saturating_sub(canon.chars().count() + 2);
        let summary = truncate(&summary, avail);
        if summary.is_empty() {
            s.push_str(&format!("{}\n", style.bold(canon)));
        } else {
            s.push_str(&format!("{}  {}\n", style.bold(canon), style.dim(&summary)));
        }
    }
    s
}

fn value_card(rec: &Record, provider: &str, width: usize, style: Style) -> String {
    let mut s = String::new();
    if let Some(dep) = &rec.deprecated {
        s.push_str(&format!("{}\n\n", style.warn(&format!("DEPRECATED: {dep}"))));
    }
    s.push_str(&identity_line(&rec.canonical, provider, width, style));
    s.push('\n');

    let block = field_block(rec, style);
    if !block.is_empty() {
        s.push('\n');
        s.push_str(&block);
    }
    let body = markdown::render(&rec.body, width, style);
    if !body.is_empty() {
        // Exactly one blank line before the body (the identity line / block
        // already end in a newline).
        s.push('\n');
        s.push_str(&body);
        s.push('\n');
    }
    s
}

fn identity_line(canonical: &str, provider: &str, width: usize, style: Style) -> String {
    let right = format!("documented by {provider}");
    let pad = width.saturating_sub(canonical.len() + right.len()).max(2);
    format!("{}{}{}", style.bold(canonical), " ".repeat(pad), style.dim(&right))
}

fn field_block(rec: &Record, style: Style) -> String {
    let mut fields: Vec<(&str, &str)> = Vec::new();
    if let Some(v) = &rec.type_ {
        fields.push(("Type", v));
    }
    if let Some(v) = &rec.default {
        fields.push(("Default", v));
    }
    if let Some(v) = &rec.valid {
        fields.push(("Valid", v));
    }
    if let Some(v) = &rec.applies {
        fields.push(("Applies", v));
    }
    if fields.is_empty() {
        return String::new();
    }
    let w = fields.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    let mut s = String::new();
    for (label, value) in fields {
        let padded = format!("{label:<w$}");
        s.push_str(&format!("  {}  {value}\n", style.bold(&padded)));
    }
    s
}

/// Truncate `s` to at most `max` characters, marking elision with `…`.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragment;
    use std::path::PathBuf;

    fn hit(text: &str) -> Hit {
        let (recs, _) = fragment::parse(text);
        Hit {
            provider: "kmes".to_string(),
            file: PathBuf::from("kmes.regman"),
            record: recs.into_iter().next().unwrap(),
        }
    }

    fn plain() -> Style {
        Style::plain()
    }

    const VALUE: &str = "\
--- machine\\system\\kmes buffercapacity
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD
default: 4194304
valid: 65536-268435456 bytes, power of two
applies: live

Per-CPU ring buffer capacity, in bytes.
";

    #[test]
    fn value_card_has_identity_fields_and_body() {
        let out = exact(&[hit(VALUE)], 78, plain());
        assert!(out.contains("Machine\\System\\KMES BufferCapacity"));
        assert!(out.contains("documented by kmes"));
        assert!(out.contains("  Type     REG_QWORD"));
        assert!(out.contains("  Default  4194304"));
        assert!(out.contains("  Applies  live"));
        assert!(out.contains("Per-CPU ring buffer capacity"));
    }

    #[test]
    fn fields_are_aligned() {
        let out = exact(&[hit(VALUE)], 78, plain());
        assert!(out.contains("  Type     REG_QWORD"));
        assert!(out.contains("  Valid    65536"));
    }

    #[test]
    fn multi_provider_shows_banner() {
        let out = exact(&[hit(VALUE), hit(VALUE)], 78, plain());
        assert!(out.starts_with("documented by 2 packages:"));
    }

    #[test]
    fn deprecated_banner_at_top() {
        let text = "\
--- a\\b foo
canonical: A\\B Foo
type: REG_DWORD
deprecated: use A\\B Bar instead

old knob
";
        let out = exact(&[hit(text)], 78, plain());
        assert!(out.starts_with("DEPRECATED: use A\\B Bar instead"));
    }

    const KEYDOC: &str = "\
--- machine\\system\\kmes
canonical: Machine\\System\\KMES

KMES configuration subtree.
";

    #[test]
    fn key_view_lists_values_with_summaries() {
        let out = key(&[hit(KEYDOC), hit(VALUE)], "Machine\\System\\KMES", 78, plain());
        assert!(out.contains("KMES configuration subtree."));
        assert!(out.contains("Values"));
        assert!(out.contains("BufferCapacity"));
        assert!(out.contains("Per-CPU ring buffer capacity, in bytes."));
    }

    #[test]
    fn key_view_without_key_doc_still_lists_values() {
        let out = key(&[hit(VALUE)], "Machine\\System\\KMES", 78, plain());
        assert!(out.contains("Machine\\System\\KMES"));
        assert!(out.contains("BufferCapacity"));
    }

    #[test]
    fn value_card_has_single_blank_line_before_body() {
        let out = exact(&[hit(VALUE)], 78, plain());
        assert!(!out.contains("\n\n\n"), "card had a double blank line:\n{out}");
    }

    #[test]
    fn truncate_adds_ellipsis() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hell…");
        assert_eq!(truncate("x", 0), "");
    }

    #[test]
    fn apropos_lists_name_and_summary() {
        let out = apropos(&[hit(VALUE)], 100, plain());
        assert!(out.contains("Machine\\System\\KMES BufferCapacity"));
        assert!(out.contains("Per-CPU ring buffer capacity, in bytes."));
        // One line per result.
        assert_eq!(out.lines().count(), 1);
    }

    #[test]
    fn body_markdown_is_rendered_not_literal() {
        let text = "\
--- a\\b foo
canonical: A\\B Foo
type: REG_DWORD

This is **important** and `code`.
";
        let plain_out = exact(&[hit(text)], 78, plain());
        assert!(plain_out.contains("This is important and code."));
        assert!(!plain_out.contains("**"));

        let color_out = exact(&[hit(text)], 78, Style::new(true));
        assert!(color_out.contains("\x1b[1mimportant\x1b[0m"));
    }
}
