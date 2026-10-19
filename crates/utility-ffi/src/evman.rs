//! C-compatible borrowed views for evman.

use crate::Bytes;

/// Resolve a name using the catalog lookup precedence.
pub const QUERY_LOOKUP: u32 = 0;

/// Match all whitespace-separated search terms.
pub const QUERY_SEARCH: u32 = 1;

/// List the first definition of every field, group, and event.
pub const QUERY_ALL: u32 = 2;

/// A field definition.
pub const RECORD_FIELD: u32 = 1;

/// A group definition.
pub const RECORD_GROUP: u32 = 2;

/// An event definition.
pub const RECORD_EVENT: u32 = 3;

/// No valid lookup result, used for null or failed handles.
pub const MATCH_INVALID: u32 = 0;

/// An exact event-name match.
pub const MATCH_EVENT: u32 = 1;

/// An exact field-name match.
pub const MATCH_FIELD: u32 = 2;

/// A field with a permitted qualifier appended.
pub const MATCH_VARIANT: u32 = 3;

/// A standard attribute of a defined field prefix.
pub const MATCH_ATTRIBUTE: u32 = 4;

/// A group or a dotted prefix containing fields or events.
pub const MATCH_INDEX: u32 = 5;

/// A set of search matches.
pub const MATCH_SEARCH: u32 = 6;

/// The complete set of first definitions.
pub const MATCH_ALL: u32 = 7;

/// A field line in an event definition.
pub const LINE_FIELD: u32 = 1;

/// An include line in a group or event definition.
pub const LINE_INCLUDE: u32 = 2;

/// A fragment-format problem rather than a numbered catalog rule.
pub const RULE_FORMAT: u32 = 0;

/// One source fragment borrowed for parsing.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SourceView {
    /// Raw Unix path; its file stem identifies the provider.
    pub path: Bytes,
    /// UTF-8 fragment text.
    pub text: Bytes,
}

/// A record borrowed from an immutable query result.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RecordView {
    /// One of the `RECORD_*` constants.
    pub kind: u32,
    /// Field path, group name, or event type, as written in the fragment.
    pub name: Bytes,
    /// Raw Unix path of the source fragment.
    pub file: Bytes,
    /// UTF-8 file stem, or `?` when the stem cannot be represented as UTF-8.
    pub provider: Bytes,
    /// Record-level Markdown prose with surrounding blank lines removed.
    pub body: Bytes,
    /// One-based source line containing the record anchor.
    pub line: u64,
}

/// A recoverable parse or validation finding borrowed from a catalog.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FindingView {
    /// Raw Unix source path associated with the finding.
    pub file: Bytes,
    /// One-based source line associated with the finding.
    pub line: u64,
    /// `RULE_FORMAT` for a format problem; otherwise the numbered catalog rule.
    pub rule: u32,
    /// UTF-8 diagnostic text.
    pub message: Bytes,
}

/// A header borrowed from a record.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HeaderView {
    /// Header name, with surrounding whitespace removed.
    pub key: Bytes,
    /// Header value, with surrounding whitespace removed.
    pub value: Bytes,
    /// One-based source line of the last assignment to this header.
    pub line: u64,
}

/// A group include or event field/include line borrowed from a result.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LineView {
    /// One of the `LINE_*` constants.
    pub kind: u32,
    /// Field path or included group name.
    pub name: Bytes,
    /// Absent for group includes; present, possibly empty text for event lines.
    pub presence: Bytes,
    /// Optional event-local `values:` text; absent for group includes.
    pub values: Bytes,
    /// Optional event-local `closed:` text; absent for group includes.
    pub closed: Bytes,
    /// One-based source line containing the field or include declaration.
    pub line: u64,
    /// Event-local Markdown explanation; absent for group includes.
    pub gloss: Bytes,
}
