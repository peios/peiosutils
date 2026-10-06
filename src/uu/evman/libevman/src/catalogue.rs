// The catalogue: every fragment's records, read as one.
//
// The rules that matter most — one definition per path, no path both a value
// and a map — cannot be checked one fragment at a time, and nor can a name be
// looked up in one: an event in kacs.evman carries fields kernel.evman
// defines. So the records of every fragment are gathered here, and each kind
// gets a table from name to its first definition. A later definition of the
// same name is kept aside as a duplicate (rule 2) and defines nothing.
//
// `resolve` is PGSS §6.4's answer to "is this path defined?": a field, a
// variant of one (`policy.generation-previous`), or a standard attribute of a
// thing or domain (`object.file.size`). The lint and the lookup share it, so
// the manual never answers for a path the lint would reject.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::corpus::{self, Source};
use crate::fragment::{self, Kind, LineKind, Record};

/// The qualifiers §6.4 lets any defined field take without a definition of
/// its own. Closed in this version.
pub const QUALIFIERS: [&str; 6] = [
    "-previous",
    "-requested",
    "-expected",
    "-effective",
    "-limit",
    "-count",
];

/// The attributes §6.4 lets any defined thing or domain take without a
/// definition of their own. Closed in this version.
pub const STANDARD_ATTRIBUTES: [&str; 22] = [
    "count",
    "limit",
    "length",
    "size",
    "depth",
    "capacity",
    "fill",
    "duration",
    "timeout",
    "attempts",
    "offset",
    "index",
    "sequence",
    "generation",
    "digest",
    "name",
    "path",
    "id",
    "guid",
    "sid",
    "type",
    "state",
];

/// One record and the fragment it came from.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The fragment name (file stem): `kacs`.
    pub fragment: String,
    pub file: PathBuf,
    pub record: Record,
}

impl Entry {
    pub fn name(&self) -> &str {
        &self.record.name
    }

    /// The file's name alone, as findings and duplicates cite it.
    pub fn file_name(&self) -> String {
        self.file
            .file_name()
            .map_or_else(|| self.file.display().to_string(), |n| n.to_string_lossy().into_owned())
    }
}

/// What a path names under §6.4.
#[derive(Debug, Clone)]
pub enum Resolution<'a> {
    /// A defined field.
    Field(&'a Entry),
    /// A defined field with a qualifier appended.
    Variant {
        field: &'a Entry,
        qualifier: &'static str,
    },
    /// A standard attribute of a thing or domain: `thing` is a map, a proper
    /// prefix of some defined field.
    Attribute { thing: String, attribute: String },
}

impl<'a> Resolution<'a> {
    /// The field record the path is defined by, when there is one; a
    /// standard attribute has none.
    pub fn field(self) -> Option<&'a Entry> {
        match self {
            Self::Field(field) | Self::Variant { field, .. } => Some(field),
            Self::Attribute { .. } => None,
        }
    }
}

/// An event type that carries a field, and how.
#[derive(Debug, Clone)]
pub struct Carrier<'a> {
    pub event: &'a Entry,
    /// The presence written on the line that carries it.
    pub presence: String,
    /// The group it arrives through, if it is not listed directly.
    pub via: Option<String>,
    /// The variant the event carries, if not the field itself.
    pub variant: Option<String>,
}

/// What `evman <name>` finds.
#[derive(Debug)]
pub enum Lookup<'a> {
    Event(&'a Entry),
    Field(&'a Entry),
    /// A variant of a defined field: its card answers for it.
    Variant {
        field: &'a Entry,
        qualifier: &'static str,
    },
    /// A standard attribute of a thing: the thing's index answers for it.
    Attribute {
        attribute: String,
        index: Index<'a>,
    },
    /// A prefix of field paths or event types, or a group's name.
    Index(Index<'a>),
}

/// What lies beneath a dotted prefix.
#[derive(Debug)]
pub struct Index<'a> {
    pub prefix: String,
    /// A group of exactly this name, if there is one.
    pub group: Option<&'a Entry>,
    pub fields: Vec<&'a Entry>,
    pub events: Vec<&'a Entry>,
}

impl Index<'_> {
    fn is_empty(&self) -> bool {
        self.group.is_none() && self.fields.is_empty() && self.events.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct Catalogue {
    entries: Vec<Entry>,
    /// First definitions of each kind, in reading order.
    fields: Vec<usize>,
    groups: Vec<usize>,
    events: Vec<usize>,
    by_name: HashMap<(Kind, String), usize>,
    /// (later definition, the one it repeats), in reading order.
    duplicates: Vec<(usize, usize)>,
    /// Every proper prefix of a defined field: the maps.
    maps: HashSet<String>,
}

impl Catalogue {
    /// Parse every source and gather the records. Framing problems are
    /// dropped here; `lint::check` reports them.
    pub fn from_sources(sources: &[Source]) -> Self {
        Self::from_parsed(
            sources
                .iter()
                .map(|s| (s.path.clone(), fragment::parse(&s.text).0))
                .collect(),
        )
    }

    /// Gather already-parsed records: (file, its records), in reading order.
    pub fn from_parsed(parsed: Vec<(PathBuf, Vec<Record>)>) -> Self {
        let mut cat = Self::default();
        for (file, records) in parsed {
            let fragment = corpus::fragment_of(&file);
            for record in records {
                let ix = cat.entries.len();
                let key = (record.kind, record.name.clone());
                if let Some(&first) = cat.by_name.get(&key) {
                    cat.duplicates.push((ix, first));
                } else {
                    cat.by_name.insert(key, ix);
                    match record.kind {
                        Kind::Field => cat.fields.push(ix),
                        Kind::Group => cat.groups.push(ix),
                        Kind::Event => cat.events.push(ix),
                    }
                }
                cat.entries.push(Entry {
                    fragment: fragment.clone(),
                    file: file.clone(),
                    record,
                });
            }
        }
        for &ix in &cat.fields {
            let name = &cat.entries[ix].record.name;
            let mut prefix = String::new();
            let parts: Vec<&str> = name.split('.').collect();
            for part in &parts[..parts.len() - 1] {
                if !prefix.is_empty() {
                    prefix.push('.');
                }
                prefix.push_str(part);
                cat.maps.insert(prefix.clone());
            }
        }
        cat
    }

    /// Every defined field, in reading order.
    pub fn fields(&self) -> impl Iterator<Item = &Entry> {
        self.fields.iter().map(|&i| &self.entries[i])
    }

    /// Every defined group, in reading order.
    pub fn groups(&self) -> impl Iterator<Item = &Entry> {
        self.groups.iter().map(|&i| &self.entries[i])
    }

    /// Every defined event type, in reading order.
    pub fn events(&self) -> impl Iterator<Item = &Entry> {
        self.events.iter().map(|&i| &self.entries[i])
    }

    /// Each repeated definition with the definition it repeats.
    pub fn duplicates(&self) -> impl Iterator<Item = (&Entry, &Entry)> {
        self.duplicates
            .iter()
            .map(|&(dup, first)| (&self.entries[dup], &self.entries[first]))
    }

    fn get(&self, kind: Kind, name: &str) -> Option<&Entry> {
        self.by_name
            .get(&(kind, name.to_string()))
            .map(|&i| &self.entries[i])
    }

    pub fn field(&self, name: &str) -> Option<&Entry> {
        self.get(Kind::Field, name)
    }

    pub fn group(&self, name: &str) -> Option<&Entry> {
        self.get(Kind::Group, name)
    }

    pub fn event(&self, name: &str) -> Option<&Entry> {
        self.get(Kind::Event, name)
    }

    /// Whether `path` is a map: a proper prefix of some defined field.
    pub fn is_map(&self, path: &str) -> bool {
        self.maps.contains(path)
    }

    /// Is `path` defined, by §6.4's reckoning? A field itself; a field with
    /// one of the free qualifiers appended; or a standard attribute of a map.
    pub fn resolve(&self, path: &str) -> Option<Resolution<'_>> {
        if let Some(field) = self.field(path) {
            return Some(Resolution::Field(field));
        }
        for qualifier in QUALIFIERS {
            if let Some(field) = path.strip_suffix(qualifier).and_then(|b| self.field(b)) {
                return Some(Resolution::Variant { field, qualifier });
            }
        }
        let (thing, attribute) = path.rsplit_once('.').unwrap_or(("", path));
        if STANDARD_ATTRIBUTES.contains(&attribute) && self.is_map(thing) {
            return Some(Resolution::Attribute {
                thing: thing.to_string(),
                attribute: attribute.to_string(),
            });
        }
        None
    }

    /// Every name a line of `event` carries: its fields, and the members of
    /// the groups it includes. An undefined group contributes nothing.
    pub fn carried(&self, event: &Record) -> HashSet<String> {
        let mut carried = HashSet::new();
        for line in &event.lines {
            match line.kind {
                LineKind::Include => {
                    if let Some(g) = self.group(&line.name) {
                        carried.extend(g.record.includes.iter().map(|(n, _)| n.clone()));
                    }
                }
                LineKind::Field => {
                    carried.insert(line.name.clone());
                }
            }
        }
        carried
    }

    /// The event types that carry `field`, directly, through a group, or as
    /// a variant, sorted by event type.
    pub fn carriers(&self, field: &str) -> Vec<Carrier<'_>> {
        let mut out = Vec::new();
        for event in self.events() {
            for line in &event.record.lines {
                match line.kind {
                    LineKind::Field if line.name == field => out.push(Carrier {
                        event,
                        presence: line.presence.clone(),
                        via: None,
                        variant: None,
                    }),
                    LineKind::Field => {
                        let is_variant = QUALIFIERS
                            .iter()
                            .any(|q| line.name.strip_suffix(q) == Some(field));
                        if is_variant && self.field(&line.name).is_none() {
                            out.push(Carrier {
                                event,
                                presence: line.presence.clone(),
                                via: None,
                                variant: Some(line.name.clone()),
                            });
                        }
                    }
                    LineKind::Include => {
                        let Some(g) = self.group(&line.name) else {
                            continue;
                        };
                        if g.record.includes.iter().any(|(n, _)| n == field) {
                            out.push(Carrier {
                                event,
                                presence: line.presence.clone(),
                                via: Some(line.name.clone()),
                                variant: None,
                            });
                        }
                    }
                }
            }
        }
        out.sort_by(|a, b| a.event.name().cmp(b.event.name()));
        out
    }

    /// What lies beneath `prefix`: the fields and event types whose names
    /// continue it past a dot, each sorted by name, and a group of exactly
    /// that name.
    pub fn index(&self, prefix: &str) -> Index<'_> {
        let below = format!("{prefix}.");
        let beneath = |e: &&Entry| e.name().starts_with(&below);
        let mut fields: Vec<&Entry> = self.fields().filter(beneath).collect();
        let mut events: Vec<&Entry> = self.events().filter(beneath).collect();
        fields.sort_by(|a, b| a.name().cmp(b.name()));
        events.sort_by(|a, b| a.name().cmp(b.name()));
        Index {
            prefix: prefix.to_string(),
            group: self.group(prefix),
            fields,
            events,
        }
    }

    /// What `name` means: an event type, a field, a prefix or group, or a
    /// path §6.4 lets a field or thing take freely — tried in that order.
    pub fn lookup(&self, name: &str) -> Option<Lookup<'_>> {
        if let Some(event) = self.event(name) {
            return Some(Lookup::Event(event));
        }
        if let Some(field) = self.field(name) {
            return Some(Lookup::Field(field));
        }
        let index = self.index(name);
        if !index.is_empty() {
            return Some(Lookup::Index(index));
        }
        match self.resolve(name)? {
            Resolution::Field(field) => Some(Lookup::Field(field)),
            Resolution::Variant { field, qualifier } => Some(Lookup::Variant { field, qualifier }),
            Resolution::Attribute { thing, attribute } => Some(Lookup::Attribute {
                attribute,
                index: self.index(&thing),
            }),
        }
    }

    /// `man -k` / apropos: every field, group and event type whose name and
    /// summary together contain all the terms, case-insensitively. Sorted by
    /// name. Like regman's, this searches the one-line summary, not the
    /// whole prose.
    pub fn apropos(&self, terms: &[String]) -> Vec<&Entry> {
        let terms: Vec<String> = terms.iter().map(|t| t.to_lowercase()).collect();
        let mut hits: Vec<&Entry> = self
            .fields()
            .chain(self.groups())
            .chain(self.events())
            .filter(|e| {
                let hay = format!("{} {}", e.name(), e.record.summary()).to_lowercase();
                terms.iter().all(|t| hay.contains(t.as_str()))
            })
            .collect();
        hits.sort_by(|a, b| {
            a.name()
                .cmp(b.name())
                .then(a.record.kind.word().cmp(b.record.kind.word()))
        });
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(name: &str, text: &str) -> Source {
        Source {
            path: PathBuf::from(format!("/x/{name}.evman")),
            text: text.to_string(),
        }
    }

    const KERNEL: &str = "\
--- field subject.token.sid
type: bin.sid

The user SID of the effective token.

--- field subject.token.groups
type: bin.sid[]

The group SIDs.

--- field policy.generation
type: uint

The generation now in force.

--- field outcome.success
type: bool

Whether the action succeeded.

--- group subject
include: subject.token.sid
include: subject.token.groups

Who acted.
";

    const KACS: &str = "\
--- event kacs.audit.access.checked
tier: essential

The record that an access check completed.

include: subject
field: outcome.success     required
field: policy.generation-previous  optional

--- event kacs.policy.loaded
tier: standard

A policy was loaded.

field: policy.generation   required
";

    fn catalogue() -> Catalogue {
        Catalogue::from_sources(&[source("kernel", KERNEL), source("kacs", KACS)])
    }

    #[test]
    fn tables_hold_first_definitions() {
        let cat = catalogue();
        assert_eq!(cat.fields().count(), 4);
        assert_eq!(cat.groups().count(), 1);
        assert_eq!(cat.events().count(), 2);
        assert_eq!(cat.field("outcome.success").unwrap().fragment, "kernel");
        assert!(cat.event("kacs.policy.loaded").is_some());
        assert!(cat.field("kacs.policy.loaded").is_none());
    }

    #[test]
    fn a_repeat_is_a_duplicate_not_a_definition() {
        let again = "--- field outcome.success\ntype: str\n\nAgain.\n";
        let cat = Catalogue::from_sources(&[source("kernel", KERNEL), source("lcs", again)]);
        assert_eq!(cat.field("outcome.success").unwrap().record.value("type"), Some("bool"));
        let dups: Vec<_> = cat.duplicates().map(|(d, f)| (d.fragment.as_str(), f.fragment.as_str())).collect();
        assert_eq!(dups, [("lcs", "kernel")]);
    }

    #[test]
    fn maps_are_proper_prefixes() {
        let cat = catalogue();
        assert!(cat.is_map("subject"));
        assert!(cat.is_map("subject.token"));
        assert!(!cat.is_map("subject.token.sid"));
    }

    #[test]
    fn resolve_follows_section_6_4() {
        let cat = catalogue();
        assert!(matches!(cat.resolve("policy.generation"), Some(Resolution::Field(_))));
        assert!(matches!(
            cat.resolve("policy.generation-previous"),
            Some(Resolution::Variant { qualifier: "-previous", .. })
        ));
        let Some(Resolution::Attribute { thing, attribute }) = cat.resolve("subject.token.type") else {
            panic!("type is a standard attribute of a map");
        };
        assert_eq!((thing.as_str(), attribute.as_str()), ("subject.token", "type"));
        // Not a standard attribute; not beneath a map; not a free qualifier.
        assert!(cat.resolve("subject.token.colour").is_none());
        assert!(cat.resolve("nowhere.size").is_none());
        assert!(cat.resolve("policy.generation-old").is_none());
    }

    #[test]
    fn carriers_cover_direct_group_and_variant() {
        let cat = catalogue();
        let c = cat.carriers("subject.token.sid");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].event.name(), "kacs.audit.access.checked");
        assert_eq!(c[0].via.as_deref(), Some("subject"));

        let c = cat.carriers("policy.generation");
        let got: Vec<_> = c.iter().map(|c| (c.event.name(), c.variant.as_deref())).collect();
        assert_eq!(
            got,
            [
                ("kacs.audit.access.checked", Some("policy.generation-previous")),
                ("kacs.policy.loaded", None)
            ]
        );
    }

    #[test]
    fn lookup_orders_exact_then_index_then_allowance() {
        let cat = catalogue();
        assert!(matches!(cat.lookup("kacs.policy.loaded"), Some(Lookup::Event(_))));
        assert!(matches!(cat.lookup("outcome.success"), Some(Lookup::Field(_))));
        let Some(Lookup::Index(ix)) = cat.lookup("subject") else {
            panic!("subject is a prefix and a group");
        };
        assert!(ix.group.is_some());
        assert_eq!(ix.fields.len(), 2);
        let Some(Lookup::Index(ix)) = cat.lookup("kacs.audit") else {
            panic!("kacs.audit is an event prefix");
        };
        assert_eq!(ix.events.len(), 1);
        assert!(matches!(cat.lookup("policy.generation-previous"), Some(Lookup::Variant { .. })));
        let Some(Lookup::Attribute { attribute, index }) = cat.lookup("subject.token.size") else {
            panic!("size is a standard attribute");
        };
        assert_eq!(attribute, "size");
        assert_eq!(index.prefix, "subject.token");
        // A prefix must end at a dot.
        assert!(cat.lookup("subject.tok").is_none());
        assert!(cat.lookup("nothing").is_none());
    }

    #[test]
    fn apropos_matches_name_and_summary_case_insensitively() {
        let cat = catalogue();
        let hits: Vec<_> = cat.apropos(&["SID".into()]).iter().map(|e| e.name()).collect();
        assert_eq!(hits, ["subject.token.groups", "subject.token.sid"]);
        let hits: Vec<_> = cat.apropos(&["access".into(), "check".into()]).iter().map(|e| e.name()).collect();
        assert_eq!(hits, ["kacs.audit.access.checked"]);
        assert!(cat.apropos(&["sid".into(), "nonsense".into()]).is_empty());
    }
}
