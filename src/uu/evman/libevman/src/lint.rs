// The §6.10 rules, checked across the whole catalogue.
//
// This is a port of `pkm/tools/evman.py lint`, which is the reference: on the
// same files it must report the same findings, in the same order, with the
// same messages. Where the Python and the prose of §6.10 part ways — rule 8's
// gating half is not mechanically checkable, rule 7's "names its reason-code
// family" is taken on trust — the Python's reading is kept, so the two stay
// interchangeable until the Python is retired.
//
// A finding is a numbered rule or a `format` problem: framing, a missing type
// or description, a malformed presence. They print as
// `file:line: rule N: message` and `file:line: format: message`.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::catalogue::{Catalogue, Entry, Resolution};
use crate::corpus::Source;
use crate::fragment::{self, LineKind};

/// The value types of §6.5. An array of any is the type with `[]`.
pub const TYPES: [&str; 20] = [
    "bin.sid",
    "bin.guid",
    "uint.luid",
    "bin.ace",
    "uint.time",
    "uint.duration",
    "uint.bytes",
    "uint.mask",
    "uint.flags",
    "uint.integrity",
    "int.errno",
    "bool",
    "str.enum",
    "uint.enum",
    "str.path",
    "str.ip",
    "bin",
    "str",
    "uint",
    "int",
];
/// The enumeration types: rule 7 asks each for its values.
pub const ENUMS: [&str; 2] = ["str.enum", "uint.enum"];
/// The tiers of §6.8, sorted as the rule 8 message lists them.
pub const TIERS: [&str; 4] = ["debug", "essential", "standard", "verbose"];

/// The participant roles of §6.4.
pub const ROLES: [&str; 5] = ["subject", "object", "source", "destination", "emitter"];
/// The generic domains of §6.4, defined only in the platform fragment.
pub const DOMAINS: [&str; 9] = [
    "event",
    "access",
    "outcome",
    "trigger",
    "operation",
    "config",
    "policy",
    "transaction",
    "fields",
];
/// The common things beneath the roles, defined only in the platform fragment.
pub const COMMON_THINGS: [&str; 5] = ["token", "process", "file", "key", "session"];

/// The platform roots of §6.A; each is owned by the fragment of that name.
pub const PLATFORM_ROOTS: [&str; 16] = [
    "kacs", "lcs", "kmes", "stratafs", "ntfe", "peinit", "peipkg", "eventd", "authd", "lpsd",
    "netd", "resolvd", "timed", "trustd", "ud", "loregd",
];
/// The platform fragment, which owns the generic roots and no event root.
pub const PLATFORM_FRAGMENT: &str = "kernel";

const FIELD_KEYS: [&str; 5] = ["type", "values", "closed", "asserted", "carried"];
const EVENT_KEYS: [&str; 3] = ["tier", "gating", "cardinality"];

/// Past-tense verbs that do not end in -ed. Rule 6 asks for a past-tense
/// verb; a regular one is recognised by its ending, these by name.
const IRREGULAR_PAST: [&str; 49] = [
    "begun", "built", "brought", "bound", "broken", "chosen", "dealt", "done", "drawn", "found",
    "frozen", "given", "gone", "held", "hidden", "kept", "known", "laid", "left", "lost", "made",
    "met", "paid", "put", "read", "run", "seen", "sent", "set", "shed", "shown", "shut", "sold",
    "split", "spent", "spun", "stood", "struck", "swept", "taken", "thrown", "torn", "told",
    "undone", "upheld", "withdrawn", "woken", "won", "written",
];

/// Which rule a finding breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// One of §6.10's nine numbered rules.
    Numbered(u8),
    /// The format itself: framing, keys, types, presence, description.
    Format,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Numbered(n) => write!(f, "rule {n}"),
            Self::Format => f.write_str("format"),
        }
    }
}

/// One broken rule, at one line of one fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub file: PathBuf,
    pub line: usize,
    pub rule: Rule,
    pub message: String,
}

impl Finding {
    /// The fragment's file name alone, as a finding cites it.
    pub fn file_name(&self) -> String {
        self.file
            .file_name()
            .map_or_else(|| self.file.display().to_string(), |n| n.to_string_lossy().into_owned())
    }
}

impl fmt::Display for Finding {
    /// `file:line: rule N: message`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}: {}", self.file_name(), self.line, self.rule, self.message)
    }
}

/// Check `sources` as one catalogue. Returns the catalogue they make, so a
/// reader can use what it has just checked, and every finding in the order
/// the reference lint reports them.
pub fn check(sources: &[Source]) -> (Catalogue, Vec<Finding>) {
    let mut findings = Vec::new();
    let mut parsed = Vec::new();
    for s in sources {
        let (records, issues) = fragment::parse(&s.text);
        findings.extend(issues.into_iter().map(|i| Finding {
            file: s.path.clone(),
            line: i.line,
            rule: Rule::Format,
            message: i.message,
        }));
        parsed.push((s.path.clone(), records));
    }
    let cat = Catalogue::from_parsed(parsed);
    Checker {
        cat: &cat,
        findings: &mut findings,
    }
    .run();
    (cat, findings)
}

/// Check `sources` as one catalogue, for the findings alone.
pub fn lint(sources: &[Source]) -> Vec<Finding> {
    check(sources).1
}

struct Checker<'a> {
    cat: &'a Catalogue,
    findings: &'a mut Vec<Finding>,
}

impl Checker<'_> {
    fn find(&mut self, at: &Entry, rule: Rule, message: String, line: Option<usize>) {
        self.findings.push(Finding {
            file: at.file.clone(),
            line: line.unwrap_or(at.record.line),
            rule,
            message,
        });
    }

    fn rule(&mut self, at: &Entry, n: u8, message: String) {
        self.find(at, Rule::Numbered(n), message, None);
    }

    fn format(&mut self, at: &Entry, message: String) {
        self.find(at, Rule::Format, message, None);
    }

    fn run(&mut self) {
        let cat = self.cat;

        // Rule 2: one definition per path.
        for (dup, first) in cat.duplicates() {
            let msg = format!(
                "{} {} is already defined at {}:{}",
                dup.record.kind.word(),
                dup.name(),
                first.file_name(),
                first.record.line
            );
            self.rule(dup, 2, msg);
        }

        // Rule 3: a path is a value or a map. Every field is a value, so no
        // field may be a proper prefix of another.
        for field in cat.fields() {
            let parts: Vec<&str> = field.name().split('.').collect();
            for i in 1..parts.len() {
                let prefix = parts[..i].join(".");
                if cat.field(&prefix).is_some() {
                    let msg = format!("{} sits beneath {prefix}, which is a value", field.name());
                    self.rule(field, 3, msg);
                }
            }
        }

        for field in cat.fields() {
            self.check_field(field);
        }

        for group in cat.groups() {
            let name = group.name();
            if !segments_ok(name) {
                self.rule(group, 6, format!("group {name}: a segment breaks the §6.3 grammar"));
            }
            for (inc, n) in &group.record.includes {
                if cat.resolve(inc).is_none() {
                    let msg = format!("group {name} includes undefined {inc}");
                    self.find(group, Rule::Numbered(1), msg, Some(*n));
                }
            }
        }

        for event in cat.events() {
            self.check_event(event);
        }
    }

    fn check_field(&mut self, field: &Entry) {
        let name = field.name();
        let rec = &field.record;
        if !segments_ok(name) {
            self.rule(field, 6, format!("{name}: a segment breaks the §6.3 grammar"));
        }
        for h in &rec.headers {
            if !FIELD_KEYS.contains(&h.key.as_str()) {
                let msg = format!("unknown field key {}", crate::py::repr(&h.key));
                self.find(field, Rule::Format, msg, Some(h.line));
            }
        }
        match rec.value("type") {
            None => self.format(field, format!("{name} declares no type")),
            Some(t) if !TYPES.contains(&t.strip_suffix("[]").unwrap_or(t)) => {
                self.format(field, format!("{name}: unknown type {}", crate::py::repr(t)));
            }
            Some(_) => {}
        }
        if let Some(carried) = rec.header("carried") {
            if carried.value != "header" {
                let msg = "carried: must be 'header'".to_string();
                self.find(field, Rule::Format, msg, Some(carried.line));
            }
        }
        let has_values = rec.header("values").is_some();
        if ENUMS.contains(&rec.base_type()) && has_values != rec.header("closed").is_some() {
            self.rule(field, 7, format!("{name} declares values or closed but not both"));
        }
        if let Some(closed) = rec.header("closed") {
            if closed.value != "true" && closed.value != "false" {
                let msg = "closed: must be true or false".to_string();
                self.find(field, Rule::Numbered(7), msg, Some(closed.line));
            }
        }
        if !rec.has_prose() {
            self.format(field, format!("{name} has no description"));
        }

        // Rule 5: who may define a path.
        let parts: Vec<&str> = name.split('.').collect();
        let top = parts[0];
        let owner = field.fragment.as_str();
        if owner == PLATFORM_FRAGMENT {
            // The platform fragment may define any path.
        } else if PLATFORM_ROOTS.contains(&owner) {
            if DOMAINS.contains(&top) {
                self.rule(
                    field,
                    5,
                    format!(
                        "{name}: {top} is a generic root, defined only in {PLATFORM_FRAGMENT}.evman"
                    ),
                );
            } else if ROLES.contains(&top) && parts.len() > 2 && COMMON_THINGS.contains(&parts[1])
            {
                self.rule(
                    field,
                    5,
                    format!(
                        "{name}: {top}.{} is a common thing, defined only in {PLATFORM_FRAGMENT}.evman",
                        parts[1]
                    ),
                );
            }
        } else {
            let package: Vec<&str> = owner.split('.').collect();
            let beneath = parts.len() > package.len() && parts[1..=package.len()] == package[..];
            if !(ROLES.contains(&top) && beneath) {
                self.rule(
                    field,
                    5,
                    format!(
                        "{name}: a package defines fields only beneath a role and its own name, <role>.{owner}.*"
                    ),
                );
            }
        }
    }

    fn check_event(&mut self, event: &Entry) {
        let cat = self.cat;
        let name = event.name();
        let rec = &event.record;
        let segs: Vec<&str> = name.split('.').collect();
        if segs.len() < 3 {
            self.rule(event, 6, format!("{name}: an event type has at least three segments"));
        }
        if name.contains('/') {
            self.rule(event, 6, format!("{name}: an event type contains no '/'"));
        }
        let owner = event.fragment.as_str();
        let body: &[&str] = if PLATFORM_ROOTS.contains(&owner) {
            if segs[0] != owner {
                self.rule(event, 4, format!("{name}: {owner}.evman owns only the {owner} root"));
            }
            &segs
        } else {
            let package: Vec<&str> = owner.split('.').collect();
            if segs.len() < package.len() || segs[..package.len()] != package[..] {
                self.rule(event, 4, format!("{name}: {owner}.evman roots its event types at {owner}"));
            }
            segs.get(package.len()..).unwrap_or(&[])
        };
        if !body.iter().all(|s| segment_ok(s)) {
            self.rule(event, 6, format!("{name}: a segment breaks the §6.3 grammar"));
        }
        if !is_past_tense(segs[segs.len() - 1]) {
            self.rule(event, 6, format!("{name}: the last segment is not a past-tense verb"));
        }
        for h in &rec.headers {
            if !EVENT_KEYS.contains(&h.key.as_str()) {
                let msg = format!("unknown event key {}", crate::py::repr(&h.key));
                self.find(event, Rule::Format, msg, Some(h.line));
            }
        }
        if !rec.value("tier").is_some_and(|t| TIERS.contains(&t)) {
            self.rule(event, 8, format!("{name}: tier must be one of {}", py_list(&TIERS)));
        }
        if !rec.has_prose() {
            self.format(event, format!("{name} has no description"));
        }

        for line in rec.lines.iter().filter(|l| l.kind == LineKind::Include) {
            if cat.group(&line.name).is_none() {
                let msg = format!("{name} includes undefined group {}", line.name);
                self.find(event, Rule::Numbered(1), msg, Some(line.line));
            }
        }
        let carried = cat.carried(rec);

        for line in rec.lines.iter().filter(|l| l.kind == LineKind::Field) {
            let n = Some(line.line);
            let Some(resolution) = cat.resolve(&line.name) else {
                let msg = format!("{name} carries undefined {}", line.name);
                self.find(event, Rule::Numbered(1), msg, n);
                continue;
            };
            let fdef = resolution.field().map(|e| &e.record);
            if fdef.is_some_and(|f| f.header("carried").is_some()) {
                let msg = format!("{} is a header field and is never listed in a payload", line.name);
                self.find(event, Rule::Format, msg, n);
            }
            let p = line.presence.as_str();
            if p.starts_with("when ") {
                match condition(p) {
                    None => {
                        let msg = format!("bad condition {}", crate::py::repr(p));
                        self.find(event, Rule::Format, msg, n);
                    }
                    Some(on) if !carried.contains(on) => {
                        let msg = format!("condition names {on}, which {name} does not carry");
                        self.find(event, Rule::Format, msg, n);
                    }
                    Some(_) => {}
                }
            } else if p != "required" && p != "optional" {
                let msg = format!(
                    "presence must be required, optional or when …, not {}",
                    crate::py::repr(p)
                );
                self.find(event, Rule::Format, msg, n);
            }
            // Rule 7 at the event: an enumeration whose field leaves its
            // values to the event must have them declared here.
            if let Some(fdef) = fdef {
                let is_enum = ENUMS.contains(&fdef.base_type());
                let field_values = fdef.header("values").is_some();
                if is_enum && !field_values {
                    if line.values.is_none() || line.closed.is_none() {
                        let msg = format!(
                            "{} leaves its values to the event, and {name} does not declare both values and closed",
                            line.name
                        );
                        self.find(event, Rule::Numbered(7), msg, n);
                    }
                } else if line.values.is_some() && is_enum && field_values {
                    let msg = format!("{} declares its own values; {name} cannot redeclare them", line.name);
                    self.find(event, Rule::Numbered(7), msg, n);
                }
            }
        }

        // Rule 9: a mask needs object.kind to decode against.
        if !carried.contains("object.kind") && carries_mask(cat, &carried) {
            self.rule(event, 9, format!("{name} carries a uint.mask and not object.kind"));
        }
    }
}

fn carries_mask(cat: &Catalogue, carried: &HashSet<String>) -> bool {
    carried.iter().any(|f| {
        cat.resolve(f)
            .and_then(Resolution::field)
            .is_some_and(|e| e.record.base_type() == "uint.mask")
    })
}

/// `[a-z][a-z0-9]*(-[a-z0-9]+)*` — the §6.3 segment grammar.
pub fn segment_ok(s: &str) -> bool {
    let b = s.as_bytes();
    let alnum = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    !b.is_empty()
        && b[0].is_ascii_lowercase()
        && b.iter().all(|&c| alnum(c) || c == b'-')
        && !s.contains("--")
        && !s.ends_with('-')
}

/// Every dot-separated segment of `name` matches the grammar.
pub fn segments_ok(name: &str) -> bool {
    name.split('.').all(segment_ok)
}

/// Whether a verb is in the past tense. A compound verb leads with the verb
/// itself: copied-up, timed-out.
pub fn is_past_tense(verb: &str) -> bool {
    let head = verb.split('-').next().unwrap_or("");
    head.ends_with("ed") || IRREGULAR_PAST.contains(&head)
}

/// `when (\S+) (==|!=|<|<=|>|>=) (\S+)` — the field a condition names, if
/// the condition is well-formed. The separators are single spaces.
fn condition(presence: &str) -> Option<&str> {
    let rest = presence.strip_prefix("when ")?;
    let end = rest.find(crate::py::is_space)?;
    let (on, rest) = (&rest[..end], &rest[end..]);
    let rest = rest.strip_prefix(' ')?;
    let ok = ["==", "!=", "<", "<=", ">", ">="].iter().any(|op| {
        rest.strip_prefix(op)
            .and_then(|r| r.strip_prefix(' '))
            .is_some_and(|v| !v.is_empty() && !v.contains(crate::py::is_space))
    });
    (end > 0 && ok).then_some(on)
}

/// A list of strings as Python prints one: `['a', 'b']`.
fn py_list(items: &[&str]) -> String {
    let quoted: Vec<String> = items.iter().map(|s| crate::py::repr(s)).collect();
    format!("[{}]", quoted.join(", "))
}

/// Read the named files and lint them as one catalogue. The files are
/// sorted and deduplicated first, as the reference lint globs its
/// directory, so the order of findings does not depend on the caller's.
pub fn lint_files(paths: &[&Path]) -> crate::error::Result<Vec<Finding>> {
    let mut paths: Vec<PathBuf> = paths.iter().map(|p| p.to_path_buf()).collect();
    paths.sort();
    paths.dedup();
    let sources = paths
        .iter()
        .map(|p| Source::read(p))
        .collect::<crate::error::Result<Vec<_>>>()?;
    Ok(lint(&sources))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal platform fragment the rule tests build on.
    const KERNEL: &str = "\
--- field subject.token.sid
type: bin.sid

The user SID.

--- field object.kind
type: str.enum
values: file | process
closed: false

What kind of object.

--- field access.granted
type: uint.mask

The mask granted.

--- field outcome.success
type: bool

Whether it worked.

--- field outcome.reason
type: str.enum

Why not.

--- field event.time
type: uint.time
carried: header

When.

--- group subject
include: subject.token.sid

Who acted.
";

    fn src(name: &str, text: &str) -> Source {
        Source {
            path: PathBuf::from(format!("/x/{name}.evman")),
            text: text.to_string(),
        }
    }

    /// Lint `kernel.evman` above plus the given fragments, as printed lines.
    fn run(extra: &[(&str, &str)]) -> Vec<String> {
        let mut sources = vec![src("kernel", KERNEL)];
        sources.extend(extra.iter().map(|(n, t)| src(n, t)));
        lint(&sources).iter().map(ToString::to_string).collect()
    }

    fn event(body: &str) -> String {
        format!("--- event kacs.thing.checked\ntier: essential\n\nAn event.\n\n{body}")
    }

    #[test]
    fn the_base_catalogue_is_clean() {
        assert_eq!(run(&[]), Vec::<String>::new());
        let ok = event("include: subject\nfield: object.kind  required\nfield: access.granted  required\n");
        assert_eq!(run(&[("kacs", &ok)]), Vec::<String>::new());
    }

    #[test]
    fn rule_1_undefined_field_group_and_include() {
        let text = format!(
            "{}\n--- group caller\ninclude: subject.token.nope\n\nWho.\n",
            event("include: nobody\nfield: subject.token.colour  optional\n")
        );
        assert_eq!(
            run(&[("kacs", &text)]),
            [
                "kacs.evman:10: rule 1: group caller includes undefined subject.token.nope",
                "kacs.evman:6: rule 1: kacs.thing.checked includes undefined group nobody",
                "kacs.evman:7: rule 1: kacs.thing.checked carries undefined subject.token.colour",
            ]
        );
    }

    #[test]
    fn rule_1_accepts_variants_and_standard_attributes() {
        let ok = event("field: outcome.success-previous  optional\nfield: subject.token.size  optional\n");
        assert_eq!(run(&[("kacs", &ok)]), Vec::<String>::new());
    }

    #[test]
    fn rule_2_duplicate_definition() {
        let text = "--- field kacs.x\ntype: str\n\nOne.\n\n--- field kacs.x\ntype: str\n\nTwo.\n";
        assert_eq!(
            run(&[("kacs", text)]),
            ["kacs.evman:6: rule 2: field kacs.x is already defined at kacs.evman:1"]
        );
        let again = "--- field outcome.success\ntype: bool\n\nAgain.\n";
        assert_eq!(
            run(&[("kernel2", again)]),
            ["kernel2.evman:1: rule 2: field outcome.success is already defined at kernel.evman:18"]
        );
    }

    #[test]
    fn rule_3_value_and_map() {
        let text = "--- field outcome.success.detail\ntype: str\n\nMore.\n";
        assert_eq!(
            run(&[("kernel2", text)])[0],
            "kernel2.evman:1: rule 3: outcome.success.detail sits beneath outcome.success, which is a value"
        );
    }

    #[test]
    fn rule_4_event_root() {
        let text = "--- event lcs.thing.checked\ntier: essential\n\nX.\n";
        assert_eq!(
            run(&[("kacs", text)]),
            ["kacs.evman:1: rule 4: lcs.thing.checked: kacs.evman owns only the kacs root"]
        );
        let text = "--- event org.other.thing.started\ntier: debug\n\nX.\n";
        assert_eq!(
            run(&[("org.jellyfin.server", text)]),
            ["org.jellyfin.server.evman:1: rule 4: org.other.thing.started: org.jellyfin.server.evman roots its event types at org.jellyfin.server"]
        );
        let text = "--- event org.jellyfin.server.playback.started\ntier: debug\n\nX.\n";
        assert_eq!(run(&[("org.jellyfin.server", text)]), Vec::<String>::new());
    }

    #[test]
    fn rule_5_who_may_define() {
        let text = "\
--- field outcome.extra
type: str

X.

--- field subject.token.extra
type: str

X.

--- field subject.stratum.index
type: uint

X.
";
        assert_eq!(
            run(&[("stratafs", text)]),
            [
                "stratafs.evman:1: rule 5: outcome.extra: outcome is a generic root, defined only in kernel.evman",
                "stratafs.evman:6: rule 5: subject.token.extra: subject.token is a common thing, defined only in kernel.evman",
            ]
        );
        let text = "--- field object.org.other.title\ntype: str\n\nX.\n\n--- field object.org.jellyfin.title\ntype: str\n\nX.\n";
        assert_eq!(
            run(&[("org.jellyfin", text)]),
            ["org.jellyfin.evman:1: rule 5: object.org.other.title: a package defines fields only beneath a role and its own name, <role>.org.jellyfin.*"]
        );
    }

    #[test]
    fn rule_6_names() {
        let text = "\
--- field kacs.Bad_name
type: str

X.

--- group Bad
include: subject.token.sid

X.

--- event kacs.short
tier: debug

X.

--- event kacs.thing/sub.checked
tier: debug

X.

--- event kacs.thing.check
tier: debug

X.

--- event kacs.thing.copied-up
tier: debug

X.

--- event kacs.thing.withdrawn
tier: debug

X.
";
        assert_eq!(
            run(&[("kacs", text)]),
            [
                "kacs.evman:1: rule 6: kacs.Bad_name: a segment breaks the §6.3 grammar",
                "kacs.evman:6: rule 6: group Bad: a segment breaks the §6.3 grammar",
                "kacs.evman:11: rule 6: kacs.short: an event type has at least three segments",
                "kacs.evman:11: rule 6: kacs.short: the last segment is not a past-tense verb",
                "kacs.evman:16: rule 6: kacs.thing/sub.checked: an event type contains no '/'",
                "kacs.evman:16: rule 6: kacs.thing/sub.checked: a segment breaks the §6.3 grammar",
                "kacs.evman:21: rule 6: kacs.thing.check: the last segment is not a past-tense verb",
            ]
        );
    }

    #[test]
    fn rule_7_enumerations() {
        let text = "\
--- field kacs.mode
type: str.enum
values: a | b

X.

--- field kacs.state
type: uint
closed: maybe

X.
";
        let ev = event(
            "field: outcome.reason  optional\n\
             field: object.kind  required\n  values: file\n  closed: true\n\
             field: outcome.reason  required\n  values: x | y\n",
        );
        assert_eq!(
            run(&[("kacs", &format!("{text}\n{ev}"))]),
            [
                "kacs.evman:1: rule 7: kacs.mode declares values or closed but not both",
                "kacs.evman:9: rule 7: closed: must be true or false",
                "kacs.evman:18: rule 7: outcome.reason leaves its values to the event, and kacs.thing.checked does not declare both values and closed",
                "kacs.evman:19: rule 7: object.kind declares its own values; kacs.thing.checked cannot redeclare them",
                "kacs.evman:22: rule 7: outcome.reason leaves its values to the event, and kacs.thing.checked does not declare both values and closed",
            ]
        );
        let ok = event("field: outcome.reason  optional\n  values: x | y\n  closed: false\n  Why.\n");
        assert_eq!(run(&[("kacs", &ok)]), Vec::<String>::new());
    }

    #[test]
    fn rule_8_tier() {
        let text = "--- event kacs.thing.checked\ntier: loud\n\nX.\n\n--- event kacs.thing.used\n\nX.\n";
        let want = "rule 8: kacs.thing.{}: tier must be one of ['debug', 'essential', 'standard', 'verbose']";
        assert_eq!(
            run(&[("kacs", text)]),
            [
                format!("kacs.evman:1: {}", want.replace("{}", "checked")),
                format!("kacs.evman:6: {}", want.replace("{}", "used")),
            ]
        );
    }

    #[test]
    fn rule_9_mask_needs_object_kind() {
        let ev = event("field: access.granted  required\n");
        assert_eq!(
            run(&[("kacs", &ev)]),
            ["kacs.evman:1: rule 9: kacs.thing.checked carries a uint.mask and not object.kind"]
        );
        // A variant of a mask is a mask.
        let ev = event("field: access.granted-previous  required\n");
        assert_eq!(run(&[("kacs", &ev)]).len(), 1);
    }

    #[test]
    fn format_framing() {
        let text = "stray\n--- record a.b\n--- field kacs.a\ntype str\n\nX.\n";
        assert_eq!(
            run(&[("kacs", text)]),
            [
                "kacs.evman:1: format: text before any record",
                "kacs.evman:2: format: bad anchor: '--- record a.b'",
                "kacs.evman:4: format: bad header: 'type str'",
                "kacs.evman:3: format: kacs.a declares no type",
            ]
        );
        let text = format!("{}stray prose\n", event("field: object.kind  required\n"));
        assert_eq!(run(&[("kacs", &text)]), ["kacs.evman:7: format: prose after the field lines"]);
    }

    #[test]
    fn format_field_records() {
        let text = "\
--- field kacs.a
type: string[]
colour: red
carried: payload
";
        assert_eq!(
            run(&[("kacs", text)]),
            [
                "kacs.evman:3: format: unknown field key 'colour'",
                "kacs.evman:1: format: kacs.a: unknown type 'string[]'",
                "kacs.evman:4: format: carried: must be 'header'",
                "kacs.evman:1: format: kacs.a has no description",
            ]
        );
    }

    #[test]
    fn format_event_records() {
        let text = "--- event kacs.thing.checked\ntier: debug\nsize: big\n\n\
                    field: object.kind  required\n\
                    field: event.time  required\n\
                    field: outcome.success  when outcome.reason = x\n\
                    field: outcome.success  when nothing == x\n\
                    field: outcome.success  sometimes\n";
        assert_eq!(
            run(&[("kacs", text)]),
            [
                "kacs.evman:3: format: unknown event key 'size'",
                "kacs.evman:1: format: kacs.thing.checked has no description",
                "kacs.evman:6: format: event.time is a header field and is never listed in a payload",
                "kacs.evman:7: format: bad condition 'when outcome.reason = x'",
                "kacs.evman:8: format: condition names nothing, which kacs.thing.checked does not carry",
                "kacs.evman:9: format: presence must be required, optional or when …, not 'sometimes'",
            ]
        );
    }

    #[test]
    fn conditions_take_every_operator() {
        for op in ["==", "!=", "<", "<=", ">", ">="] {
            assert_eq!(condition(&format!("when a.b {op} 3")), Some("a.b"), "{op}");
        }
        assert_eq!(condition("when a.b  == 3"), None);
        assert_eq!(condition("when a.b == 3 4"), None);
        assert_eq!(condition("when a.b =="), None);
        assert_eq!(condition("when a.b"), None);
    }

    #[test]
    fn grammar_and_tense() {
        for ok in ["a", "kacs", "copied-up", "a1-b2", "x9"] {
            assert!(segment_ok(ok), "{ok}");
        }
        for bad in ["", "A", "1a", "a_b", "a-", "a--b", "-a"] {
            assert!(!segment_ok(bad), "{bad}");
        }
        assert!(is_past_tense("checked"));
        assert!(is_past_tense("copied-up"));
        assert!(is_past_tense("withdrawn"));
        assert!(!is_past_tense("check"));
    }

    #[test]
    fn files_are_read_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.evman");
        let b = tmp.path().join("b.evman");
        std::fs::write(&a, "x\n").unwrap();
        std::fs::write(&b, "y\n").unwrap();
        let got: Vec<String> = lint_files(&[b.as_path(), a.as_path(), b.as_path()])
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            got,
            ["a.evman:1: format: text before any record", "b.evman:1: format: text before any record"]
        );
    }
}
