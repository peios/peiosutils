//! C-compatible borrowed views for regman.

use crate::Bytes;

/// Match a key or value anchor, including documented wildcard patterns.
pub const QUERY_EXACT: u32 = 0;
/// Match a key and its directly attached value documentation.
pub const QUERY_KEY: u32 = 1;
/// Match all whitespace-separated terms against names and summaries.
pub const QUERY_SEARCH: u32 = 2;

/// A registry-manual record borrowed from an immutable result.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RecordView {
    /// Fragment provider derived from its filename; empty for parsed text.
    pub provider: Bytes,
    /// Raw Unix source path; empty for parsed text.
    pub file: Bytes,
    /// Key/value anchor as written; conforming fragments use case-folded text.
    pub anchor: Bytes,
    /// Canonical key or key/value spelling from the fragment.
    pub canonical: Bytes,
    /// Optional type declaration for a value.
    pub type_name: Bytes,
    /// Optional default-value description, as written.
    pub default_value: Bytes,
    /// Optional description of accepted values.
    pub valid: Bytes,
    /// Optional description of when changes take effect.
    pub applies: Bytes,
    /// Optional deprecation information.
    pub deprecated: Bytes,
    /// Markdown prose with surrounding blank lines removed.
    pub body: Bytes,
    /// Zero-based byte offset of the record fence within the UTF-8 source.
    pub fence_offset: u64,
}

/// A parser diagnostic at a one-based source line.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IssueView {
    /// One-based source line associated with the problem.
    pub line: u64,
    /// UTF-8 diagnostic text.
    pub message: Bytes,
}
