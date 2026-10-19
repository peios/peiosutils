//! Event catalog parsing, validation and lookup backed by libevman.so.

use std::ffi::{OsString, c_void};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use utility_client::{Handle, copy_bytes, copy_text, optional_text, view};
use utility_ffi::{
    Bytes,
    evman::{self, SourceView, RecordView, FindingView, HeaderView, LineView},
};

pub use utility_client::Error;

#[link(name = "evman")]
unsafe extern "C" {
    fn peios_evman_load(directory: Bytes) -> *mut c_void;
    fn peios_evman_parse(sources: *const SourceView, count: usize) -> *mut c_void;
    fn peios_evman_query(handle: *const c_void, mode: u32, text: Bytes) -> *mut c_void;
    fn peios_evman_catalog_free(handle: *mut c_void);
    fn peios_evman_catalog_error(handle: *const c_void) -> i32;
    fn peios_evman_catalog_message(handle: *const c_void) -> Bytes;
    fn peios_evman_matches_free(handle: *mut c_void);
    fn peios_evman_matches_error(handle: *const c_void) -> i32;
    fn peios_evman_matches_message(handle: *const c_void) -> Bytes;
    fn peios_evman_finding_count(handle: *const c_void) -> usize;
    fn peios_evman_finding(handle: *const c_void, index: usize, out: *mut FindingView) -> i32;
    fn peios_evman_record_count(handle: *const c_void) -> usize;
    fn peios_evman_record(handle: *const c_void, index: usize, out: *mut RecordView) -> i32;
    fn peios_evman_match_kind(handle: *const c_void) -> u32;
    fn peios_evman_qualifier(handle: *const c_void) -> Bytes;
    fn peios_evman_attribute(handle: *const c_void) -> Bytes;
    fn peios_evman_prefix(handle: *const c_void) -> Bytes;
    fn peios_evman_header_count(handle: *const c_void, record: usize) -> usize;
    fn peios_evman_header(
        handle: *const c_void,
        record: usize,
        index: usize,
        out: *mut HeaderView,
    ) -> i32;
    fn peios_evman_include_count(handle: *const c_void, record: usize) -> usize;
    fn peios_evman_include(
        handle: *const c_void,
        record: usize,
        index: usize,
        out: *mut LineView,
    ) -> i32;
    fn peios_evman_line_count(handle: *const c_void, record: usize) -> usize;
    fn peios_evman_line(
        handle: *const c_void,
        record: usize,
        index: usize,
        out: *mut LineView,
    ) -> i32;
}

/// One input fragment, identified by a raw Unix path whose stem is its provider.
#[derive(Debug, Clone, Copy)]
pub struct Source<'a> {
    /// Source identity; its file stem names the provider and need not exist on disk.
    pub path: &'a Path,
    /// UTF-8 fragment text, borrowed only for the parse call.
    pub text: &'a str,
}

/// An immutable catalog. Findings are separate from operation errors.
pub struct Catalog(Handle);

/// A numbered catalog rule, or a format problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// The fragment could not be parsed as written.
    Format,
    /// A numbered event-catalog rule was violated.
    Numbered(u32),
}

/// A recoverable fragment-format or cross-source validation problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Source path retaining its original Unix bytes.
    pub file: PathBuf,
    /// One-based source line associated with the problem.
    pub line: u64,
    /// Format classification or numbered catalog rule.
    pub rule: Rule,
    /// Diagnostic text explaining the problem.
    pub message: String,
}

/// The kind of a catalog definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A reusable field definition.
    Field,
    /// A named group of included fields.
    Group,
    /// An event type and its payload declarations.
    Event,
}

/// How a catalog query resolved its input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// An exact event type.
    Event,
    /// An exact field path.
    Field,
    /// A defined field with a permitted qualifier appended.
    Variant,
    /// A standard attribute of a defined field prefix.
    Attribute,
    /// A group or dotted prefix containing fields/events.
    Index,
    /// Records matching every search term.
    Search,
    /// All first definitions in the catalog.
    All,
}

/// The meaning of a field/include declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// An event field declaration.
    Field,
    /// A group member or an event inclusion of a group.
    Include,
}

/// A record header after whitespace trimming and duplicate-key resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Header name, with surrounding whitespace removed.
    pub key: String,
    /// Last assigned value, with surrounding whitespace removed.
    pub value: String,
    /// One-based source line of the last assignment.
    pub line: u64,
}

/// A group include or event field/include declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Whether this declaration names a field or includes a group/field.
    pub kind: LineKind,
    /// Field path or included group name.
    pub name: String,
    /// `None` for group includes; `Some`, possibly empty, for event lines.
    pub presence: Option<String>,
    /// Optional event-local `values:` text; `None` for group includes.
    pub values: Option<String>,
    /// Optional event-local `closed:` text; `None` for group includes.
    pub closed: Option<String>,
    /// One-based source line containing the declaration.
    pub line: u64,
    /// Event-local Markdown explanation; `None` for group includes.
    pub gloss: Option<String>,
}

/// An owned field, group, or event definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Which kind of catalog definition this record describes.
    pub kind: Kind,
    /// Field path, group name, or event type, as written.
    pub name: String,
    /// Source path retaining its original Unix bytes.
    pub file: PathBuf,
    /// Source file stem, or `?` when the stem is not valid UTF-8.
    pub provider: String,
    /// Record-level Markdown prose with surrounding blank lines removed.
    pub body: String,
    /// One-based source line of the record anchor.
    pub line: u64,
    /// Headers in first-seen order; repeated keys retain their last assigned value.
    pub headers: Vec<Header>,
    /// Group includes in source order; empty for other record kinds.
    pub includes: Vec<Line>,
    /// Event field/include declarations in source order; empty for other kinds.
    pub lines: Vec<Line>,
}

/// Owned lookup metadata and records. These can outlive the source catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matches {
    /// How the lookup or query produced these records.
    pub kind: MatchKind,
    /// Appended qualifier for a variant match; empty for other match kinds.
    pub qualifier: String,
    /// Standard attribute for an attribute match; empty otherwise.
    pub attribute: String,
    /// Dotted prefix for index/attribute results; empty for other match kinds.
    pub prefix: String,
    /// Owned record copies in the ordering documented by the query method.
    pub records: Vec<Record>,
}

/// Copy a raw Unix path borrowed from a library result.
///
/// # Safety
/// The span must identify a readable libevman-produced view whose immutable
/// owner remains alive throughout this call.
unsafe fn path(value: Bytes) -> PathBuf {
    // SAFETY: callers retain the result owning this raw Unix path view during the copy.
    PathBuf::from(OsString::from_vec(unsafe { copy_bytes(value) }))
}

impl Catalog {
    /// Take ownership of a catalog reply, releasing failures automatically.
    ///
    /// # Safety
    /// A non-null pointer must be a live libevman catalog reply. Its unique
    /// destruction right transfers to this call, including on an error return.
    unsafe fn adopt(raw: *mut c_void) -> Result<Self, Error> {
        // SAFETY: internal callers transfer a new catalog reply from the matching library.
        unsafe {
            Handle::adopt(
                raw,
                peios_evman_catalog_free,
                peios_evman_catalog_error,
                peios_evman_catalog_message,
            )
        }
        .map(Self)
    }

    /// Load the directory, or select EVMAN_DIR and the built-in default when None.
    ///
    /// A missing directory is an empty catalog. Other read failures return EIO.
    /// Format and validation problems are available through [`Self::findings`].
    pub fn load(directory: Option<&Path>) -> Result<Self, Error> {
        let directory = directory.map_or_else(Bytes::absent, |path| {
            Bytes::new(path.as_os_str().as_bytes())
        });
        // SAFETY: the path remains borrowed for the call; adopt owns the returned catalog.
        unsafe { Self::adopt(peios_evman_load(directory)) }
    }

    /// Parse sources together, retaining cross-source validation findings.
    ///
    /// Source paths identify providers; this operation does not open them. Inputs
    /// are copied and need not outlive this call. Parse/validation findings do not
    /// fail the operation; inspect [`Self::findings`] separately.
    pub fn parse(sources: &[Source<'_>]) -> Result<Self, Error> {
        let views: Vec<_> = sources
            .iter()
            .map(|source| SourceView {
                path: Bytes::new(source.path.as_os_str().as_bytes()),
                text: Bytes::text(source.text),
            })
            .collect();
        // SAFETY: the array and all borrowed spans remain valid for the call.
        unsafe { Self::adopt(peios_evman_parse(views.as_ptr(), views.len())) }
    }

    /// Return owned validation findings, including parser format problems.
    pub fn findings(&self) -> Result<Vec<Finding>, Error> {
        let mut result = Vec::new();
        // SAFETY: all getters and views borrow this live successful catalog.
        unsafe {
            for index in 0..peios_evman_finding_count(self.0.as_ptr()) {
                let finding = view(|out| peios_evman_finding(self.0.as_ptr(), index, out))?;
                result.push(Finding {
                    file: path(finding.file),
                    line: finding.line,
                    rule: if finding.rule == evman::RULE_FORMAT {
                        Rule::Format
                    } else {
                        Rule::Numbered(finding.rule)
                    },
                    message: copy_text(finding.message),
                });
            }
        }
        Ok(result)
    }

    /// Resolve an event, field, group/prefix, variant, or attribute, in that order.
    ///
    /// A missing name returns ENOENT. Index/attribute results place a matching group
    /// first, then fields sorted by name, then events sorted by name. All returned
    /// strings and records are owned and can outlive this catalog.
    pub fn lookup(&self, name: &str) -> Result<Matches, Error> {
        self.query(evman::QUERY_LOOKUP, name)
    }

    /// Find records whose names and summaries match every whitespace-separated term.
    ///
    /// Matching is case-insensitive. Results sort by name, then record-kind word.
    /// No matches produce an empty result rather than an error.
    pub fn search(&self, terms: &str) -> Result<Matches, Error> {
        self.query(evman::QUERY_SEARCH, terms)
    }

    /// List the first definition of each field, group and event, in that order.
    pub fn entries(&self) -> Result<Matches, Error> {
        self.query(evman::QUERY_ALL, "")
    }

    fn query(&self, mode: u32, text: &str) -> Result<Matches, Error> {
        // SAFETY: the live catalog and text are borrowed for this call.
        let raw = unsafe { peios_evman_query(self.0.as_ptr(), mode, Bytes::text(text)) };
        // SAFETY: raw is a new query reply from the matching shared library.
        let reply = unsafe {
            Handle::adopt(
                raw,
                peios_evman_matches_free,
                peios_evman_matches_error,
                peios_evman_matches_message,
            )
        }?;
        // SAFETY: the successful reply owns all views until collect finishes copying them.
        unsafe { Matches::collect(&reply) }
    }
}

impl Line {
    /// Copy a borrowed field/include line into owned Rust fields.
    ///
    /// # Safety
    /// The view must come from libevman, and its immutable owner must remain
    /// alive throughout this call. Every present text span must contain UTF-8.
    unsafe fn copy(value: LineView) -> Self {
        // SAFETY: all spans borrow a retained successful query result.
        unsafe {
            Self {
                kind: match value.kind {
                    evman::LINE_FIELD => LineKind::Field,
                    evman::LINE_INCLUDE => LineKind::Include,
                    _ => unreachable!("libevman returned an invalid line kind"),
                },
                name: copy_text(value.name),
                presence: optional_text(value.presence),
                values: optional_text(value.values),
                closed: optional_text(value.closed),
                line: value.line,
                gloss: optional_text(value.gloss),
            }
        }
    }
}

impl Record {
    /// Copy one query record and its nested fields.
    ///
    /// # Safety
    /// The handle must own a successful libevman matches reply. It must not be
    /// freed or mutated outside the Rust owner while these views are copied.
    unsafe fn collect(handle: &Handle, index: usize) -> Result<Self, Error> {
        // SAFETY: all getters and views borrow this retained successful query result.
        unsafe {
            let value = view(|out| peios_evman_record(handle.as_ptr(), index, out))?;
            let mut headers = Vec::new();
            let mut includes = Vec::new();
            let mut lines = Vec::new();
            for header in 0..peios_evman_header_count(handle.as_ptr(), index) {
                let item = view(|out| peios_evman_header(handle.as_ptr(), index, header, out))?;
                headers.push(Header {
                    key: copy_text(item.key),
                    value: copy_text(item.value),
                    line: item.line,
                });
            }
            for include in 0..peios_evman_include_count(handle.as_ptr(), index) {
                includes.push(Line::copy(view(|out| {
                    peios_evman_include(handle.as_ptr(), index, include, out)
                })?));
            }
            for line in 0..peios_evman_line_count(handle.as_ptr(), index) {
                lines.push(Line::copy(view(|out| {
                    peios_evman_line(handle.as_ptr(), index, line, out)
                })?));
            }
            Ok(Self {
                kind: match value.kind {
                    evman::RECORD_FIELD => Kind::Field,
                    evman::RECORD_GROUP => Kind::Group,
                    evman::RECORD_EVENT => Kind::Event,
                    _ => unreachable!("libevman returned an invalid record kind"),
                },
                name: copy_text(value.name),
                file: path(value.file),
                provider: copy_text(value.provider),
                body: copy_text(value.body),
                line: value.line,
                headers,
                includes,
                lines,
            })
        }
    }
}

impl Matches {
    /// Copy query metadata and records into an independent Rust result.
    ///
    /// # Safety
    /// The handle must own a successful libevman matches reply. It must not be
    /// freed or mutated outside the Rust owner while these views are copied.
    unsafe fn collect(handle: &Handle) -> Result<Self, Error> {
        // SAFETY: all getters and views borrow this retained successful query result.
        unsafe {
            let kind = match peios_evman_match_kind(handle.as_ptr()) {
                evman::MATCH_EVENT => MatchKind::Event,
                evman::MATCH_FIELD => MatchKind::Field,
                evman::MATCH_VARIANT => MatchKind::Variant,
                evman::MATCH_ATTRIBUTE => MatchKind::Attribute,
                evman::MATCH_INDEX => MatchKind::Index,
                evman::MATCH_SEARCH => MatchKind::Search,
                evman::MATCH_ALL => MatchKind::All,
                _ => unreachable!("libevman returned an invalid lookup kind"),
            };
            let mut records = Vec::new();
            for index in 0..peios_evman_record_count(handle.as_ptr()) {
                records.push(Record::collect(handle, index)?);
            }
            Ok(Self {
                kind,
                qualifier: copy_text(peios_evman_qualifier(handle.as_ptr())),
                attribute: copy_text(peios_evman_attribute(handle.as_ptr())),
                prefix: copy_text(peios_evman_prefix(handle.as_ptr())),
                records,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use super::{Catalog, Source};

    #[test]
    fn empty_catalog_is_valid_and_lookup_reports_missing() {
        let catalog = Catalog::parse(&[]).unwrap();
        assert!(catalog.entries().unwrap().records.is_empty());
        assert!(catalog.lookup("missing").is_err());
    }

    #[test]
    fn malformed_source_reports_findings_without_panicking() {
        let catalog = Catalog::parse(&[Source {
            path: Path::new("example.evman"),
            text: "--- field example.value\ntype: invalid\n\nA value.\n",
        }])
        .unwrap();
        assert!(!catalog.findings().unwrap().is_empty());
        let records = catalog.entries().unwrap();
        drop(catalog);
        assert_eq!(records.records[0].name, "example.value");
    }

    #[test]
    fn event_gloss_is_owned_and_group_include_fields_are_absent() {
        let text = concat!(
            "--- field example.value\ntype: str\n\nA value.\n\n",
            "--- group example\ninclude: example.value\n\nA group.\n\n",
            "--- event example.updated\ntier: standard\n\nAn update.\n\n",
            "include: example\nfield: example.value optional\n",
            "  First paragraph.\n\n  Second paragraph.\n",
            "field: example.value optional\n",
        );
        let catalog = Catalog::parse(&[Source {
            path: Path::new("example.evman"),
            text,
        }])
        .unwrap();
        let event = catalog.lookup("example.updated").unwrap();
        let group = catalog.lookup("example").unwrap();
        drop(catalog);
        let lines = &event.records[0].lines;
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].gloss.as_deref(), Some(""));
        assert_eq!(lines[0].presence.as_deref(), Some(""));
        assert_eq!(
            lines[1].gloss.as_deref(),
            Some("First paragraph.\n\nSecond paragraph.")
        );
        assert_eq!(lines[2].gloss.as_deref(), Some(""));
        let include = &group.records[0].includes[0];
        assert!(include.gloss.is_none());
        assert!(include.presence.is_none());
        assert!(include.values.is_none());
        assert!(include.closed.is_none());
    }
}
