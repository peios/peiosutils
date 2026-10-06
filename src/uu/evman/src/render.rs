// Card rendering.
//
// Like regman, evman renders a consistent card rather than `man`'s freeform
// sections: identity line → aligned header block (only what is present) →
// prose → a list. An event card lists the fields the event carries, with any
// group expanded under its name; a field card lists the event types that
// carry it, which is the reverse index no single fragment holds. A prefix
// renders as an index, one summary line per name beneath it, the summary
// being the prose's first sentence (§6.10).
//
// These functions are pure and take an explicit `width` and `Style` so output
// is deterministic and testable; tty concerns (paging) live in the pager.

use crate::catalogue::{Catalogue, Entry, Index};
use crate::fragment::{FieldLine, LineKind};
use crate::lint::ENUMS;
use crate::markdown::{self, Style};

/// An event type's card: identity, headers, prose, then its fields.
pub fn event(cat: &Catalogue, entry: &Entry, width: usize, style: Style) -> String {
    let rec = &entry.record;
    let mut s = identity_line(entry.name(), &entry.fragment, width, style);
    s.push('\n');

    let rows: Vec<(&str, String)> = [("Tier", "tier"), ("Gating", "gating"), ("Cardinality", "cardinality")]
        .into_iter()
        .filter_map(|(label, key)| rec.value(key).map(|v| (label, v.to_string())))
        .collect();
    push_block(&mut s, &rows, width, style);
    push_body(&mut s, &rec.body(), width, style);

    if rec.lines.is_empty() {
        return s;
    }
    s.push_str(&format!("\n{}\n", style.bold("Fields")));

    // One name column for the whole list, group members indented within it.
    let col = rec
        .lines
        .iter()
        .flat_map(|l| {
            let members = match (l.kind, cat.group(&l.name)) {
                (LineKind::Include, Some(g)) => g.record.includes.iter().map(|(m, _)| m.len() + 2).collect(),
                _ => Vec::new(),
            };
            members.into_iter().chain([l.name.len()])
        })
        .max()
        .unwrap_or(0);

    for line in &rec.lines {
        match line.kind {
            LineKind::Include => {
                let group = cat.group(&line.name);
                let label = match group {
                    Some(g) => format!("group, from {}", g.fragment),
                    None => "group, undefined".to_string(),
                };
                let presence = join_nonempty(&line.presence, &style.dim(&label));
                s.push_str(&format!("  {}  {presence}\n", style.bold(&pad(&line.name, col))));
                if let Some(g) = group {
                    for (member, _) in &g.record.includes {
                        s.push_str(&format!("    {}\n", pad(member, col - 2).trim_end()));
                    }
                }
            }
            LineKind::Field => {
                let mut presence = line.presence.clone();
                if cat.resolve(&line.name).is_none() {
                    presence = join_nonempty(&presence, &style.dim("undefined"));
                }
                s.push_str(&format!("  {}  {presence}\n", style.bold(&pad(&line.name, col))));
                push_line_detail(&mut s, line, width, style);
            }
        }
    }
    s
}

/// A field's card: identity, headers, prose, then the event types that carry
/// it. `note` heads the card when the name asked for was not the field's own.
pub fn field(cat: &Catalogue, entry: &Entry, note: Option<&str>, width: usize, style: Style) -> String {
    let rec = &entry.record;
    let mut s = String::new();
    push_note(&mut s, note, width, style);
    s.push_str(&identity_line(entry.name(), &entry.fragment, width, style));
    s.push('\n');

    let mut rows: Vec<(&str, String)> = Vec::new();
    if let Some(t) = rec.value("type") {
        rows.push(("Type", t.to_string()));
    }
    match rec.value("values") {
        Some(v) => rows.push(("Values", v.to_string())),
        None if ENUMS.contains(&rec.base_type()) => {
            rows.push(("Values", "declared by each event that carries it".to_string()));
        }
        None => {}
    }
    if let Some(v) = rec.value("closed") {
        rows.push(("Closed", v.to_string()));
    }
    if let Some(v) = rec.value("asserted") {
        rows.push(("Asserted", v.to_string()));
    }
    if let Some(v) = rec.value("carried") {
        rows.push(("Carried", v.to_string()));
    }
    push_block(&mut s, &rows, width, style);
    push_body(&mut s, &rec.body(), width, style);

    s.push_str(&format!("\n{}\n", style.bold("Carried by")));
    if rec.header("carried").is_some() {
        s.push_str("  every event, in the record header\n");
        return s;
    }
    let carriers = cat.carriers(entry.name());
    if carriers.is_empty() {
        s.push_str(&format!("  {}\n", style.dim("no event type")));
        return s;
    }
    let col = carriers.iter().map(|c| c.event.name().len()).max().unwrap_or(0);
    for c in &carriers {
        let mut how = c.presence.clone();
        if let Some(group) = &c.via {
            how = join_nonempty(&how, &style.dim(&format!("(via group {group})")));
        }
        if let Some(variant) = &c.variant {
            how = join_nonempty(&how, &style.dim(&format!("(as {variant})")));
        }
        s.push_str(&format!("  {}  {how}\n", style.bold(&pad(c.event.name(), col))));
    }
    s
}

/// What lies beneath a prefix: a group of that name, if there is one, then
/// the fields and the event types beneath it, one summary line each.
pub fn index(cat: &Catalogue, index: &Index, note: Option<&str>, width: usize, style: Style) -> String {
    let mut s = String::new();
    push_note(&mut s, note, width, style);

    if let Some(g) = index.group {
        s.push_str(&identity_line(&format!("{} (group)", g.name()), &g.fragment, width, style));
        s.push('\n');
        push_body(&mut s, &g.record.body(), width, style);
        let members: Vec<(&str, String)> = g
            .record
            .includes
            .iter()
            .map(|(m, _)| {
                let summary = cat.field(m).map(|f| f.record.summary()).unwrap_or_default();
                (m.as_str(), summary)
            })
            .collect();
        push_list(&mut s, "Includes", &members, width, style);
    } else {
        s.push_str(&style.bold(&index.prefix));
        s.push('\n');
    }

    push_list(&mut s, "Fields", &summarise(&index.fields), width, style);
    push_list(&mut s, "Event types", &summarise(&index.events), width, style);
    s
}

fn summarise<'a>(entries: &[&'a Entry]) -> Vec<(&'a str, String)> {
    entries.iter().map(|e| (e.name(), e.record.summary())).collect()
}

/// Render apropos (`-k`) results: one line per match, `name  summary`, the
/// summary truncated to fit. Like `man -k`, with the name in place of
/// `name(section)`.
pub fn apropos(hits: &[&Entry], width: usize, style: Style) -> String {
    let mut s = String::new();
    for h in hits {
        let name = h.name();
        let summary = markdown::strip_inline(&h.record.summary());
        let avail = width.saturating_sub(name.chars().count() + 2);
        let summary = truncate(&summary, avail);
        if summary.is_empty() {
            s.push_str(&format!("{}\n", style.bold(name)));
        } else {
            s.push_str(&format!("{}  {}\n", style.bold(name), style.dim(&summary)));
        }
    }
    s
}

fn identity_line(name: &str, fragment: &str, width: usize, style: Style) -> String {
    let right = format!("defined by {fragment}");
    let pad = width.saturating_sub(name.len() + right.len()).max(2);
    format!("{}{}{}", style.bold(name), " ".repeat(pad), style.dim(&right))
}

/// A note above a card saying how the name asked for led to it.
fn push_note(s: &mut String, note: Option<&str>, width: usize, style: Style) {
    if let Some(note) = note {
        for line in wrap_words(note, width) {
            s.push_str(&style.dim(&line));
            s.push('\n');
        }
        s.push('\n');
    }
}

/// The aligned header block, preceded by a blank line when there is one.
/// A long value wraps under itself.
fn push_block(s: &mut String, rows: &[(&str, String)], width: usize, style: Style) {
    if rows.is_empty() {
        return;
    }
    s.push('\n');
    let w = rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    for (label, value) in rows {
        let lines = wrap_words(value, width.saturating_sub(w + 4));
        for (i, line) in lines.iter().enumerate() {
            let label = if i == 0 { pad(label, w) } else { " ".repeat(w) };
            s.push_str(&format!("  {}  {line}\n", style.bold(&label)));
        }
    }
}

/// The prose, preceded by exactly one blank line.
fn push_body(s: &mut String, body: &str, width: usize, style: Style) {
    let body = markdown::render(body, width, style);
    if !body.is_empty() {
        s.push('\n');
        s.push_str(&body);
        s.push('\n');
    }
}

/// A titled list of `name  summary` lines, each kept to one line.
fn push_list(s: &mut String, title: &str, items: &[(&str, String)], width: usize, style: Style) {
    if items.is_empty() {
        return;
    }
    s.push_str(&format!("\n{}\n", style.bold(title)));
    let w = items.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    let avail = width.saturating_sub(w + 4);
    for (name, summary) in items {
        let summary = truncate(&markdown::strip_inline(summary), avail);
        let line = format!("  {}  {summary}", style.bold(&pad(name, w)));
        s.push_str(line.trim_end());
        s.push('\n');
    }
}

/// An event's per-field detail, indented under its line: declared values,
/// then the gloss.
fn push_line_detail(s: &mut String, line: &FieldLine, width: usize, style: Style) {
    const INDENT: &str = "      ";
    let inner = width.saturating_sub(INDENT.len());
    let mut rows: Vec<(&str, String)> = Vec::new();
    if let Some((v, _)) = &line.values {
        rows.push(("Values", v.clone()));
    }
    if let Some((v, _)) = &line.closed {
        rows.push(("Closed", v.clone()));
    }
    let w = rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    for (label, value) in &rows {
        for (i, text) in wrap_words(value, inner.saturating_sub(w + 2)).iter().enumerate() {
            let label = if i == 0 { pad(label, w) } else { " ".repeat(w) };
            s.push_str(&format!("{INDENT}{}  {text}\n", style.dim(&label)));
        }
    }
    let gloss = markdown::render(&line.gloss(), inner, style);
    for text in gloss.lines() {
        if text.is_empty() {
            s.push('\n');
        } else {
            s.push_str(&format!("{INDENT}{text}\n"));
        }
    }
}

fn pad(s: &str, w: usize) -> String {
    format!("{s:<w$}")
}

fn join_nonempty(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else {
        format!("{a}  {b}")
    }
}

/// Greedy word wrap to `width` columns; a word longer than the width stands
/// on its own line. A lone `|`, the separator of a values list, stays at the
/// end of the line with the value before it. Always at least one line.
fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut words: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match words.last_mut() {
            Some(last) if word == "|" => last.push_str(" |"),
            _ => words.push(word.to_string()),
        }
    }
    let mut lines = vec![String::new()];
    for word in &words {
        let cur = lines.last_mut().expect("never empty");
        if cur.is_empty() {
            cur.push_str(word);
        } else if cur.chars().count() + 1 + word.chars().count() <= width {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(word.clone());
        }
    }
    lines
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
    use crate::catalogue::Lookup;
    use crate::corpus::Source;
    use std::path::PathBuf;

    const KERNEL: &str = "\
--- field subject.token.sid
type: bin.sid

The user SID of the effective token. More detail follows.

--- field subject.token.integrity
type: uint.integrity

The integrity RID.

--- field object.kind
type: str.enum
values: file | process
closed: false

What kind of object the event is about.

--- field operation.name
type: str.enum

What was attempted.

--- field access.granted
type: uint.mask

The mask granted.

--- field policy.generation
type: uint

The generation now in force.

--- field event.time
type: uint.time
carried: header

When the record was made.

--- group subject
include: subject.token.sid
include: subject.token.integrity

The identity of whoever acted.
";

    const KACS: &str = "\
--- event kacs.audit.handle.used
tier: standard
gating: the alarm mask on the handle
cardinality: once per operation

The record of what was done with a handle. It is **silent** by default.

include: subject
field: object.kind              required
field: operation.name           required
  values: file.access | file.mmap
  closed: false
field: access.granted           required
  What the handle was opened with.
field: policy.generation-previous  optional
";

    fn catalogue() -> Catalogue {
        let src = |n: &str, t: &str| Source {
            path: PathBuf::from(format!("/x/{n}.evman")),
            text: t.to_string(),
        };
        Catalogue::from_sources(&[src("kernel", KERNEL), src("kacs", KACS)])
    }

    fn plain() -> Style {
        Style::plain()
    }

    #[test]
    fn event_card_has_headers_prose_and_fields() {
        let cat = catalogue();
        let out = event(&cat, cat.event("kacs.audit.handle.used").unwrap(), 78, plain());
        assert!(out.starts_with("kacs.audit.handle.used"));
        assert!(out.lines().next().unwrap().ends_with("defined by kacs"));
        assert!(out.contains("  Tier         standard"));
        assert!(out.contains("  Gating       the alarm mask on the handle"));
        assert!(out.contains("  Cardinality  once per operation"));
        assert!(out.contains("It is silent by default."));
        assert!(out.contains("\nFields\n"));
        assert!(!out.contains("\n\n\n"), "double blank line:\n{out}");
    }

    #[test]
    fn event_card_expands_groups_and_shows_detail() {
        let cat = catalogue();
        let out = event(&cat, cat.event("kacs.audit.handle.used").unwrap(), 78, plain());
        let lines: Vec<&str> = out.lines().collect();
        let at = lines.iter().position(|l| l.starts_with("  subject ")).unwrap();
        assert!(lines[at].ends_with("group, from kernel"));
        assert_eq!(lines[at + 1].trim(), "subject.token.sid");
        assert!(lines[at + 1].starts_with("    "));
        assert_eq!(lines[at + 2].trim(), "subject.token.integrity");
        assert!(out.contains("      Values  file.access | file.mmap"));
        assert!(out.contains("      Closed  false"));
        assert!(out.contains("      What the handle was opened with."));
        assert!(out.lines().any(|l| l.starts_with("  object.kind") && l.ends_with("required")));
    }

    #[test]
    fn field_card_has_headers_and_carriers() {
        let cat = catalogue();
        let out = field(&cat, cat.field("subject.token.sid").unwrap(), None, 78, plain());
        assert!(out.starts_with("subject.token.sid"));
        assert!(out.contains("  Type  bin.sid"));
        assert!(out.contains("The user SID of the effective token."));
        assert!(out.contains("\nCarried by\n"));
        assert!(out.contains("kacs.audit.handle.used  (via group subject)"));
    }

    #[test]
    fn field_card_for_an_open_enumeration() {
        let cat = catalogue();
        let out = field(&cat, cat.field("operation.name").unwrap(), None, 78, plain());
        assert!(out.contains("  Values  declared by each event that carries it"));
        let out = field(&cat, cat.field("object.kind").unwrap(), None, 78, plain());
        assert!(out.contains("  Values  file | process"));
        assert!(out.contains("  Closed  false"));
    }

    #[test]
    fn header_field_is_carried_by_every_event() {
        let cat = catalogue();
        let out = field(&cat, cat.field("event.time").unwrap(), None, 78, plain());
        assert!(out.contains("  Carried  header"));
        assert!(out.contains("every event, in the record header"));
    }

    #[test]
    fn variant_card_carries_its_note() {
        let cat = catalogue();
        let Some(Lookup::Variant { field: f, .. }) = cat.lookup("policy.generation-previous") else {
            panic!("a variant");
        };
        let out = field(&cat, f, Some("a note"), 78, plain());
        assert!(out.starts_with("a note\n\npolicy.generation "));
        assert!(out.contains("kacs.audit.handle.used  optional  (as policy.generation-previous)"));
    }

    #[test]
    fn index_lists_summaries_one_line_each() {
        let cat = catalogue();
        let out = index(&cat, &cat.index("subject.token"), None, 78, plain());
        assert!(out.starts_with("subject.token\n"));
        assert!(out.contains("\nFields\n"));
        assert!(out.contains("  subject.token.sid        The user SID of the effective token.\n"));
        assert!(!out.contains("More detail follows"));
        let out = index(&cat, &cat.index("kacs"), None, 78, plain());
        assert!(out.contains("\nEvent types\n"));
        assert!(out.contains("kacs.audit.handle.used  The record of what was done with a handle."));
    }

    #[test]
    fn index_of_a_group_shows_the_group() {
        let cat = catalogue();
        let out = index(&cat, &cat.index("subject"), None, 78, plain());
        assert!(out.starts_with("subject (group)"));
        assert!(out.contains("The identity of whoever acted."));
        assert!(out.contains("\nIncludes\n"));
        assert!(out.contains("\nFields\n"));
    }

    #[test]
    fn apropos_lists_name_and_summary() {
        let cat = catalogue();
        let hits = cat.apropos(&["handle".to_string()]);
        let out = apropos(&hits, 100, plain());
        assert_eq!(out, "kacs.audit.handle.used  The record of what was done with a handle.\n");
    }

    #[test]
    fn markup_is_rendered_not_literal() {
        let cat = catalogue();
        let entry = cat.event("kacs.audit.handle.used").unwrap();
        assert!(!event(&cat, entry, 78, plain()).contains("**"));
        assert!(event(&cat, entry, 78, Style::new(true)).contains("\x1b[1msilent\x1b[0m"));
    }

    #[test]
    fn long_values_wrap_under_themselves() {
        let lines = wrap_words("one two three", 7);
        assert_eq!(lines, ["one two", "three"]);
        let lines = wrap_words("a | b | c | d", 7);
        assert_eq!(lines, ["a | b |", "c | d"]);
        assert_eq!(wrap_words("", 5), [""]);
    }

    #[test]
    fn truncate_adds_ellipsis() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hell…");
        assert_eq!(truncate("x", 0), "");
    }
}
