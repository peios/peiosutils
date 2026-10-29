//! Owned Rust records backed by libregman.so parsing and lookup.

use std::ffi::{OsString, c_void};
use std::ptr;
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
    fn peios_regman_fold(text: Bytes, out: *mut u8, capacity: usize, length: *mut usize) -> i32;
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

/// Whether a record documents a key or a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A key, which carries no value metadata.
    Key,
    /// A value, which carries a type, default, accepted values or when a change applies.
    Value,
}

impl Record {
    /// Whether the record documents a key or a value: a value's record carries
    /// at least one field only values have.
    pub fn kind(&self) -> Kind {
        if self.type_name.is_some()
            || self.default_value.is_some()
            || self.valid.is_some()
            || self.applies.is_some()
        {
            Kind::Value
        } else {
            Kind::Key
        }
    }

    /// The first non-empty line of the body: the one-line summary.
    pub fn summary(&self) -> &str {
        self.body
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("")
    }

    /// The value's name, without the canonical path of the key `key` it is
    /// under, compared as manual names compare; when the record's canonical
    /// name does not begin with that key, the name after its last space.
    pub fn value_name(&self, key: &str) -> Result<&str, Error> {
        let prefix = key.len() + 1;
        if self.canonical.len() > prefix
            && self.canonical.is_char_boundary(key.len())
            && self.canonical.is_char_boundary(prefix)
            && self.canonical.as_bytes()[key.len()] == b' '
            && fold(&self.canonical[..key.len()])? == fold(key)?
        {
            return Ok(&self.canonical[prefix..]);
        }
        Ok(self
            .canonical
            .rsplit_once(' ')
            .map_or(self.canonical.as_str(), |(_, name)| name))
    }
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

/// `text` case-folded as manual names compare: the folding a query applies to
/// its text, so that names compare as the library compares them.
pub fn fold(text: &str) -> Result<String, Error> {
    let mut length = 0usize;
    // SAFETY: text is borrowed for the call; zero capacity with a null output
    // only asks for the folded length, which the library writes to `length`.
    let code = unsafe { peios_regman_fold(Bytes::text(text), ptr::null_mut(), 0, &raw mut length) };
    if code != 0 && code != libc::ERANGE {
        return Err(Error::from_code(code));
    }
    let mut out = vec![0u8; length];
    // SAFETY: out is writable for its whole length, the length the library asked for.
    let code = unsafe {
        peios_regman_fold(
            Bytes::text(text),
            out.as_mut_ptr(),
            out.len(),
            &raw mut length,
        )
    };
    if code != 0 {
        return Err(Error::from_code(code));
    }
    out.truncate(length);
    String::from_utf8(out).map_err(|_| Error::from_code(libc::EINVAL))
}

/// Look up a key and its documented values in the installed manuals: the
/// directory and index `REGMAN_DIR` and `REGMAN_INDEX` name, or their
/// defaults.
pub fn resolve_key_default(path: &str) -> Result<Manual, Error> {
    query(Locations::default(), Query::Key, path)
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

    #[test]
    fn names_fold_and_records_say_their_kind() {
        assert_eq!(super::fold("Machine\\KMES").unwrap(), "machine\\kmes");
        let manual = super::parse(
            "--- machine\\kmes\ncanonical: Machine\\KMES\n\nThe subtree.\n\n\
             --- machine\\kmes buffer capacity\ncanonical: Machine\\KMES Buffer Capacity\n\
             type: REG_QWORD\n\nRing buffer capacity.\n",
        )
        .unwrap();
        assert_eq!(manual.records[0].kind(), super::Kind::Key);
        assert_eq!(manual.records[1].kind(), super::Kind::Value);
        assert_eq!(
            manual.records[1].value_name("machine\\kmes").unwrap(),
            "Buffer Capacity"
        );
        assert_eq!(manual.records[1].summary(), "Ring buffer capacity.");
    }
}
