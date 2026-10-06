// Fragment data model + parser.
//
// A fragment (`/usr/share/evman/<component>.evman`) is a sequence of records
// (PGSS §6.10). Each opens with an anchor line naming its kind and the name
// it defines, then `key: value` headers, a blank line, and prose:
//
//     --- event kacs.audit.access.checked
//     tier: essential
//                                      <- blank line ends the headers
//     The record that an access check completed, and what it decided.
//
//     include: subject                 <- an event's field lines follow its
//     field: access.granted  required     prose
//       The mask this check granted.   <- an indented gloss
//
// Unlike a regman fence the anchor is the name itself, so there is nothing
// folded to bake and no `fmt`. The parser follows `pkm/tools/evman.py` line
// for line, because `evman lint` has to report what that reports: every
// framing problem is an issue, the parse carries on past it, and Python's
// own string semantics are used where they decide a line (see `py`). Where
// the Python drops something the lint never looks at — an event field's
// gloss — this keeps it, for rendering.

use crate::py;

/// The three record kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Field,
    Group,
    Event,
}

impl Kind {
    fn from_word(word: &str) -> Option<Self> {
        match word {
            "field" => Some(Self::Field),
            "group" => Some(Self::Group),
            "event" => Some(Self::Event),
            _ => None,
        }
    }

    /// The word the anchor spells it with.
    pub fn word(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Group => "group",
            Self::Event => "event",
        }
    }
}

/// One `key: value` header line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub key: String,
    pub value: String,
    /// 1-based line number in the fragment.
    pub line: usize,
}

/// Whether an event's line names a field or a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Field,
    Include,
}

/// A `field:` or `include:` line of an event record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLine {
    pub kind: LineKind,
    /// The field path or group name.
    pub name: String,
    /// `required`, `optional`, `when <condition>`, or whatever was written
    /// (empty on a plain `include:` line).
    pub presence: String,
    pub line: usize,
    /// An indented `values:` line: the values the event declares for a
    /// field that leaves them to the event, with its line number.
    pub values: Option<(String, usize)>,
    /// An indented `closed:` line, with its line number.
    pub closed: Option<(String, usize)>,
    /// The rest of the indented paragraph: what the field means here.
    gloss: Vec<String>,
}

impl FieldLine {
    /// The gloss as Markdown, leading and trailing blank lines trimmed.
    pub fn gloss(&self) -> String {
        join_trimmed(&self.gloss)
    }
}

/// A field, group or event record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub kind: Kind,
    pub name: String,
    /// Line number of the anchor.
    pub line: usize,
    /// Headers in first-seen order. A repeated key keeps its first position
    /// and takes the later value, as a Python dict assignment does.
    pub headers: Vec<Header>,
    /// A group's `include:` lines: (field path, line).
    pub includes: Vec<(String, usize)>,
    /// An event's `field:` and `include:` lines.
    pub lines: Vec<FieldLine>,
    prose: Vec<String>,
}

impl Record {
    fn new(kind: Kind, name: &str, line: usize) -> Self {
        Self {
            kind,
            name: name.to_string(),
            line,
            headers: Vec::new(),
            includes: Vec::new(),
            lines: Vec::new(),
            prose: Vec::new(),
        }
    }

    /// The header named `key`, if the record has one.
    pub fn header(&self, key: &str) -> Option<&Header> {
        self.headers.iter().find(|h| h.key == key)
    }

    /// The value of the header named `key`.
    pub fn value(&self, key: &str) -> Option<&str> {
        self.header(key).map(|h| h.value.as_str())
    }

    /// A field's declared type with any array suffix `[]` removed: the type
    /// the enumeration and mask rules look at.
    pub fn base_type(&self) -> &str {
        let t = self.value("type").unwrap_or("");
        t.strip_suffix("[]").unwrap_or(t)
    }

    /// Whether the record has a description at all.
    pub fn has_prose(&self) -> bool {
        self.prose.iter().any(|l| !py::strip(l).is_empty())
    }

    /// The prose as Markdown, leading and trailing blank lines trimmed.
    pub fn body(&self) -> String {
        join_trimmed(&self.prose)
    }

    /// The prose's first sentence: the one-line summary an index shows.
    /// §6.10 requires it to be a complete summary of the record.
    pub fn summary(&self) -> String {
        first_sentence(&self.body())
    }

    fn set_header(&mut self, key: &str, value: &str, line: usize) {
        if let Some(h) = self.headers.iter_mut().find(|h| h.key == key) {
            h.value = value.to_string();
            h.line = line;
        } else {
            self.headers.push(Header {
                key: key.to_string(),
                value: value.to_string(),
                line,
            });
        }
    }
}

/// A framing problem found while parsing a fragment. The lint reports each
/// as a `format` finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseIssue {
    pub line: usize,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Headers,
    Prose,
    Fields,
}

/// Parse a fragment's text into records, with every framing problem found
/// along the way. A malformed anchor opens no record, so the lines up to the
/// next good anchor have nowhere to go and are each reported in turn.
pub fn parse(text: &str) -> (Vec<Record>, Vec<ParseIssue>) {
    let mut records: Vec<Record> = Vec::new();
    let mut issues = Vec::new();
    // Whether the last record in `records` is the one being read.
    let mut open = false;
    let mut state = State::Headers;

    for (i, raw) in py::splitlines(text).into_iter().enumerate() {
        let n = i + 1;
        let mut issue = |message: String| issues.push(ParseIssue { line: n, message });

        if let Some(rest) = raw.strip_prefix("--- ") {
            let parts = py::split_ws(rest);
            let kind = parts.first().and_then(|w| Kind::from_word(w));
            if let ([_, name], Some(kind)) = (parts.as_slice(), kind) {
                records.push(Record::new(kind, name, n));
                open = true;
                state = State::Headers;
            } else {
                issue(format!("bad anchor: {}", py::repr(raw)));
                open = false;
            }
            continue;
        }
        let rec = match records.last_mut() {
            Some(rec) if open => rec,
            _ => {
                if !py::strip(raw).is_empty() {
                    issue("text before any record".to_string());
                }
                continue;
            }
        };

        if state == State::Headers {
            if py::strip(raw).is_empty() {
                state = State::Prose;
            } else if let Some((key, value)) = raw.split_once(':') {
                // The group test is on the key as written, unstripped.
                if rec.kind == Kind::Group && key == "include" {
                    rec.includes.push((py::strip(value).to_string(), n));
                } else {
                    rec.set_header(py::strip(key), py::strip(value), n);
                }
            } else {
                issue(format!("bad header: {}", py::repr(raw)));
            }
            continue;
        }

        if rec.kind == Kind::Event {
            if let Some((kind, name, presence)) = field_line(raw) {
                rec.lines.push(FieldLine {
                    kind,
                    name: name.to_string(),
                    presence: presence.to_string(),
                    line: n,
                    values: None,
                    closed: None,
                    gloss: Vec::new(),
                });
                state = State::Fields;
                continue;
            }
        }

        if state == State::Fields {
            // Only a `field:`/`include:` line sets this state, so there is
            // always a line to attach to.
            let last = rec.lines.last_mut().expect("fields state has a line");
            if raw.starts_with("  ") {
                match sub_header(raw) {
                    Some(("values", v)) => last.values = Some((v.to_string(), n)),
                    Some((_, v)) => last.closed = Some((v.to_string(), n)),
                    None => last.gloss.push(py::strip(raw).to_string()),
                }
            } else if !py::strip(raw).is_empty() {
                issue("prose after the field lines".to_string());
            } else if !last.gloss.is_empty() {
                // A blank line between field lines: a paragraph break, should
                // the gloss go on after it.
                last.gloss.push(String::new());
            }
            continue;
        }

        rec.prose.push(raw.to_string());
    }

    (records, issues)
}

/// `(field|include): (\S+)\s*(.*)` — the kind, the name, and the presence
/// with surrounding whitespace removed.
fn field_line(raw: &str) -> Option<(LineKind, &str, &str)> {
    let (kind, rest) = if let Some(rest) = raw.strip_prefix("field: ") {
        (LineKind::Field, rest)
    } else if let Some(rest) = raw.strip_prefix("include: ") {
        (LineKind::Include, rest)
    } else {
        return None;
    };
    let end = rest.find(py::is_space).unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some((kind, &rest[..end], py::strip(&rest[end..])))
}

/// `\s+(values|closed): (.*)` — an event's per-field `values:` or `closed:`
/// line, with its value stripped.
fn sub_header(raw: &str) -> Option<(&'static str, &str)> {
    let rest = raw.trim_start_matches(py::is_space);
    if rest.len() == raw.len() {
        return None;
    }
    for key in ["values", "closed"] {
        if let Some(v) = rest.strip_prefix(key).and_then(|r| r.strip_prefix(": ")) {
            return Some((key, py::strip(v)));
        }
    }
    None
}

fn join_trimmed(lines: &[String]) -> String {
    let blank = |l: &&String| py::strip(l).is_empty();
    let start = lines.iter().position(|l| !blank(&l)).unwrap_or(lines.len());
    let end = lines.iter().rposition(|l| !blank(&l)).map_or(start, |e| e + 1);
    lines[start..end].join("\n")
}

/// The first sentence of `body`'s first paragraph, on one line. A sentence
/// ends at `.`, `?` or `!` followed by whitespace or the end of the
/// paragraph; a dot inside a code span (`` `kacs.audit` ``) does not end one.
fn first_sentence(body: &str) -> String {
    let para: Vec<&str> = body
        .lines()
        .map(str::trim)
        .take_while(|l| !l.is_empty())
        .collect();
    let para = para.join(" ");
    let mut in_code = false;
    let mut chars = para.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '`' => in_code = !in_code,
            '.' | '?' | '!'
                if !in_code && chars.peek().is_none_or(|&(_, next)| next.is_whitespace()) =>
            {
                return para[..=i].to_string();
            }
            _ => {}
        }
    }
    para
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIELD: &str = "\
--- field subject.token.sid
type: bin.sid

The user SID of the effective token. Under impersonation this is the
client's SID.
";

    const EVENT: &str = "\
--- event kacs.audit.handle.used
tier: standard
gating: an alarm mask

The record of what was done with a handle.

include: subject
field: object.kind              required
  Always `file` today.
field: operation.name           required
  values: file.access | file.mmap
  closed: false
field: outcome.reason           when outcome.success == false
  values: signed-exec
  closed: false
  From `KACS_FSR_*`.

  A second paragraph.
";

    #[test]
    fn parses_field_record() {
        let (recs, issues) = parse(FIELD);
        assert!(issues.is_empty());
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r.kind, Kind::Field);
        assert_eq!(r.name, "subject.token.sid");
        assert_eq!(r.line, 1);
        assert_eq!(r.value("type"), Some("bin.sid"));
        assert!(r.body().starts_with("The user SID"));
        assert_eq!(r.summary(), "The user SID of the effective token.");
    }

    #[test]
    fn parses_event_lines_values_and_glosses() {
        let (recs, issues) = parse(EVENT);
        assert!(issues.is_empty(), "{issues:?}");
        let r = &recs[0];
        assert_eq!(r.kind, Kind::Event);
        assert_eq!(r.value("gating"), Some("an alarm mask"));
        assert_eq!(r.body(), "The record of what was done with a handle.");
        let names: Vec<_> = r.lines.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["subject", "object.kind", "operation.name", "outcome.reason"]);
        assert_eq!(r.lines[0].kind, LineKind::Include);
        assert_eq!(r.lines[0].presence, "");
        assert_eq!(r.lines[1].presence, "required");
        assert_eq!(r.lines[1].gloss(), "Always `file` today.");
        assert_eq!(r.lines[2].values, Some(("file.access | file.mmap".to_string(), 11)));
        assert_eq!(r.lines[2].closed, Some(("false".to_string(), 12)));
        assert_eq!(r.lines[2].gloss(), "");
        assert_eq!(r.lines[3].presence, "when outcome.success == false");
        assert_eq!(r.lines[3].gloss(), "From `KACS_FSR_*`.\n\nA second paragraph.");
    }

    #[test]
    fn group_collects_includes() {
        let text = "--- group subject\ninclude: subject.token.sid\ninclude: subject.token.groups\n\nWho acted.\n";
        let (recs, issues) = parse(text);
        assert!(issues.is_empty());
        let names: Vec<_> = recs[0].includes.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["subject.token.sid", "subject.token.groups"]);
        assert!(recs[0].headers.is_empty());
    }

    #[test]
    fn repeated_header_keeps_position_takes_value() {
        let text = "--- field a.b\ncolour: red\ntype: str\ncolour: blue\n\nx\n";
        let (recs, _) = parse(text);
        let keys: Vec<_> = recs[0].headers.iter().map(|h| (h.key.as_str(), h.value.as_str(), h.line)).collect();
        assert_eq!(keys, [("colour", "blue", 4), ("type", "str", 3)]);
    }

    #[test]
    fn bad_anchor_orphans_the_lines_after_it() {
        let text = "--- widget a.b\ntype: str\n\nprose\n--- field c.d\ntype: str\n\nok\n";
        let (recs, issues) = parse(text);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].name, "c.d");
        let got: Vec<_> = issues.iter().map(|i| (i.line, i.message.as_str())).collect();
        assert_eq!(
            got,
            [
                (1, "bad anchor: '--- widget a.b'"),
                (2, "text before any record"),
                (4, "text before any record"),
            ]
        );
    }

    #[test]
    fn bare_dashes_are_text_not_an_anchor() {
        let (recs, issues) = parse("---\n");
        assert!(recs.is_empty());
        assert_eq!(issues[0].message, "text before any record");
    }

    #[test]
    fn bad_header_and_prose_after_field_lines() {
        let text = "--- event a.b.created\ntier standard\n\nx\n\nfield: c.d required\nstray\n";
        let (_, issues) = parse(text);
        let got: Vec<_> = issues.iter().map(|i| (i.line, i.message.as_str())).collect();
        assert_eq!(
            got,
            [(2, "bad header: 'tier standard'"), (7, "prose after the field lines")]
        );
    }

    #[test]
    fn field_lines_belong_to_events_only() {
        let text = "--- field a.b\ntype: str\n\nfield: c.d required\n";
        let (recs, issues) = parse(text);
        assert!(issues.is_empty());
        assert!(recs[0].lines.is_empty());
        assert_eq!(recs[0].body(), "field: c.d required");
    }

    #[test]
    fn crlf_is_one_line_break() {
        let (recs, issues) = parse("--- field a.b\r\ntype: str\r\n\r\nx\r\n");
        assert!(issues.is_empty());
        assert_eq!(recs[0].value("type"), Some("str"));
        assert!(recs[0].has_prose());
    }

    #[test]
    fn summary_skips_dots_in_code_spans() {
        let text = "--- field event.type\ntype: str\n\nThe event type, such as `kacs.audit.access.checked`. Stamped\nfrom the argument.\n";
        let (recs, _) = parse(text);
        assert_eq!(recs[0].summary(), "The event type, such as `kacs.audit.access.checked`.");
    }

    #[test]
    fn summary_joins_wrapped_lines() {
        assert_eq!(first_sentence("One sentence that\nwraps. Two."), "One sentence that wraps.");
        assert_eq!(first_sentence("No full stop\n\nNext paragraph."), "No full stop");
        assert_eq!(first_sentence(""), "");
    }
}
