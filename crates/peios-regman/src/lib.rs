//! Owned Rust records backed by libregman.so parsing and lookup.

use std::ffi::{OsString, c_void};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use utility_client::{Handle, copy_bytes, copy_text, optional_text, view};
use utility_ffi::{
    Bytes,
    regman::{self, IssueView, RecordView},
};

pub use utility_client::Error;

#[link(name = "regman")]
unsafe extern "C" {
    fn peios_regman_parse(source: Bytes) -> *mut c_void;
    fn peios_regman_query(directory: Bytes, index: Bytes, mode: u32, text: Bytes) -> *mut c_void;
    fn peios_regman_free(handle: *mut c_void);
    fn peios_regman_error(handle: *const c_void) -> i32;
    fn peios_regman_message(handle: *const c_void) -> Bytes;
    fn peios_regman_record_count(handle: *const c_void) -> usize;
    fn peios_regman_issue_count(handle: *const c_void) -> usize;
    fn peios_regman_record(handle: *const c_void, index: usize, out: *mut RecordView) -> i32;
    fn peios_regman_issue(handle: *const c_void, index: usize, out: *mut IssueView) -> i32;
}

/// One documented key or value. Optional fields preserve absent versus empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Fragment provider derived from its filename; empty when parsing source text.
    pub provider: String,
    /// Source path with its original Unix bytes; empty when parsing source text.
    pub file: PathBuf,
    /// Key/value anchor as written; conforming fragments use case-folded text.
    pub anchor: String,
    /// Canonical spelling of the key or key/value pair.
    pub canonical: String,
    /// Optional value type declaration.
    pub type_name: Option<String>,
    /// Optional default-value description, preserved as text.
    pub default_value: Option<String>,
    /// Optional description of accepted values.
    pub valid: Option<String>,
    /// Optional description of when changes take effect.
    pub applies: Option<String>,
    /// Optional deprecation information.
    pub deprecated: Option<String>,
    /// Markdown description with surrounding blank lines removed.
    pub body: String,
    /// Zero-based byte offset of the record fence within the UTF-8 source.
    pub fence_offset: u64,
}

/// A recoverable parser problem at a one-based source line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// One-based source line associated with the problem.
    pub line: u64,
    /// Diagnostic text explaining the rejected or malformed input.
    pub message: String,
}

/// Results remain owned and usable after the shared-library handle is released.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manual {
    /// Matching or successfully parsed records.
    pub records: Vec<Record>,
    /// Recoverable parse problems; file-based query results currently leave this empty.
    pub issues: Vec<Issue>,
}

/// How a query matches documentation.
#[derive(Debug, Clone, Copy)]
pub enum Query {
    /// Match a key or value anchor, honoring wildcard specificity.
    Exact,
    /// Match a key and its directly attached values, honoring wildcard specificity.
    Key,
    /// Match all terms against canonical names and one-line summaries.
    Search,
}

/// Optional corpus locations. None selects the library's environment/default path.
#[derive(Debug, Clone, Copy, Default)]
pub struct Locations<'a> {
    /// Manual-fragment directory; `None` selects REGMAN_DIR or the library default.
    pub directory: Option<&'a Path>,
    /// Index file; `None` selects REGMAN_INDEX or the library default.
    pub index: Option<&'a Path>,
}

fn path(value: Option<&Path>) -> Bytes {
    value.map_or_else(Bytes::absent, |path| {
        Bytes::new(path.as_os_str().as_bytes())
    })
}

impl Record {
    /// Copy a borrowed record into independent Rust-owned fields.
    ///
    /// # Safety
    /// The view must come from libregman, and its immutable owner must remain
    /// alive throughout this call. Text fields must contain UTF-8; the file field
    /// contains raw Unix path bytes.
    unsafe fn copy(value: RecordView) -> Self {
        // SAFETY: every field is a valid view borrowed from the retained reply.
        unsafe {
            Self {
                provider: copy_text(value.provider),
                file: PathBuf::from(OsString::from_vec(copy_bytes(value.file))),
                anchor: copy_text(value.anchor),
                canonical: copy_text(value.canonical),
                type_name: optional_text(value.type_name),
                default_value: optional_text(value.default_value),
                valid: optional_text(value.valid),
                applies: optional_text(value.applies),
                deprecated: optional_text(value.deprecated),
                body: copy_text(value.body),
                fence_offset: value.fence_offset,
            }
        }
    }
}

/// Adopt a manual reply and copy all of its records and diagnostics.
///
/// # Safety
/// A non-null pointer must be a live libregman result. Its unique destruction
/// right transfers to this call, including on an error return.
unsafe fn collect(raw: *mut c_void) -> Result<Manual, Error> {
    // SAFETY: callers transfer a new libregman result and its matching operations.
    let handle = unsafe {
        Handle::adopt(
            raw,
            peios_regman_free,
            peios_regman_error,
            peios_regman_message,
        )
    }?;
    let mut records = Vec::new();
    let mut issues = Vec::new();
    // SAFETY: the handle remains live and successful for all accessor calls and view copies.
    unsafe {
        for index in 0..peios_regman_record_count(handle.as_ptr()) {
            let item = view(|out| peios_regman_record(handle.as_ptr(), index, out))?;
            records.push(Record::copy(item));
        }
        for index in 0..peios_regman_issue_count(handle.as_ptr()) {
            let item = view(|out| peios_regman_issue(handle.as_ptr(), index, out))?;
            issues.push(Issue {
                line: item.line,
                message: copy_text(item.message),
            });
        }
    }
    Ok(Manual { records, issues })
}

/// Parse a fragment. Malformed records appear in issues beside the valid records.
///
/// The result owns its strings and can outlive `source`. Fragment diagnostics do
/// not fail the operation; callers must inspect [`Manual::issues`].
pub fn parse(source: &str) -> Result<Manual, Error> {
    // SAFETY: source is borrowed only during the call; collect adopts the returned allocation.
    unsafe { collect(peios_regman_parse(Bytes::text(source))) }
}

/// Look up documentation. The library folds query case; search splits terms on whitespace.
///
/// Exact/key matches prefer the most specific documentation for each record kind.
/// A missing or invalid index falls back to reading the corpus. No match produces
/// an empty result. Unreadable fragments produce a POSIX-coded error.
/// Returned records own their data independently of the query and library reply.
pub fn query(locations: Locations<'_>, kind: Query, text: &str) -> Result<Manual, Error> {
    let mode = match kind {
        Query::Exact => regman::QUERY_EXACT,
        Query::Key => regman::QUERY_KEY,
        Query::Search => regman::QUERY_SEARCH,
    };
    // SAFETY: input paths and text are borrowed for the call; collect owns the returned reply.
    unsafe {
        collect(peios_regman_query(
            path(locations.directory),
            path(locations.index),
            mode,
            Bytes::text(text),
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parsed_records_and_issues_are_owned() {
        let manual =
            super::parse("--- machine\\example\ncanonical: Machine\\Example\n\nDescription.\n")
                .unwrap();
        assert_eq!(manual.records.len(), 1);
        assert_eq!(manual.records[0].canonical, "Machine\\Example");
        assert_eq!(manual.records[0].body, "Description.");
        assert!(manual.records[0].type_name.is_none());
        assert!(manual.issues.is_empty());
    }
}
