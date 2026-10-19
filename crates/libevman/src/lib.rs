//! C interface for event catalogs. See include/peios/evman.h.
//!
//! All non-null input pointers must remain readable for the given lengths.
//! Handles are immutable. Their views remain valid until the owning handle is
//! freed. Read-only calls can run concurrently; destruction requires exclusivity.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use event_catalogue::{catalogue, corpus, fragment, lint};
use utility_ffi::{Bytes, Fault, Reply, bytes, text, evman};

pub use utility_ffi::evman::{SourceView, RecordView, FindingView, HeaderView, LineView};

/// Immutable parsed catalog and its recoverable validation findings.
pub struct Catalog {
    value: catalogue::Catalogue,
    findings: Vec<lint::Finding>,
}

struct Record {
    entry: catalogue::Entry,
    body: String,
    line_glosses: Vec<String>,
}

/// Independently owned query records and lookup metadata.
pub struct Matches {
    records: Vec<Record>,
    kind: u32,
    qualifier: String,
    attribute: String,
    prefix: String,
}

/// Owned catalog operation result, released by `peios_evman_catalog_free`.
pub type CatalogReply = Reply<Catalog>;
/// Owned query result, released by `peios_evman_matches_free`.
pub type MatchesReply = Reply<Matches>;

impl Matches {
    fn new(kind: u32) -> Self {
        Self {
            records: Vec::new(),
            kind,
            qualifier: String::new(),
            attribute: String::new(),
            prefix: String::new(),
        }
    }

    fn add(&mut self, entry: &catalogue::Entry) {
        self.records.push(Record {
            entry: entry.clone(),
            body: entry.record.body(),
            line_glosses: entry
                .record
                .lines
                .iter()
                .map(fragment::FieldLine::gloss)
                .collect(),
        });
    }

    fn index(&mut self, index: catalogue::Index<'_>) {
        self.prefix = index.prefix;
        if let Some(group) = index.group {
            self.add(group);
        }
        for entry in index.fields.into_iter().chain(index.events) {
            self.add(entry);
        }
    }
}

fn catalog(sources: Vec<corpus::Source>) -> Catalog {
    let (value, findings) = lint::check(&sources);
    Catalog { value, findings }
}

/// Return the C ABI major version.
#[unsafe(no_mangle)]
pub extern "C" fn peios_evman_abi_version() -> u32 {
    1
}

/// Load and validate a catalog directory. An absent path selects EVMAN_DIR or its default.
/// # Safety
/// The directory span must contain readable raw Unix path bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_load(directory: Bytes) -> *mut CatalogReply {
    let result = (|| {
        // SAFETY: directory follows the exported borrowed span contract.
        let path_bytes = unsafe { bytes(directory)? };
        let directory = if directory.data.is_null() {
            corpus::dir()
        } else {
            PathBuf::from(OsStr::from_bytes(path_bytes))
        };
        corpus::read(&directory)
            .map(catalog)
            .map_err(|error| Fault {
                code: libc::EIO,
                message: error.to_string(),
            })
    })();
    Reply::finish(result)
}

/// Parse and validate sources together; source paths identify their component fragments.
/// Validation findings are returned alongside the usable catalog, not as an operation error.
/// # Safety
/// Sources must be a readable, aligned array. Its spans must be valid for their lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_parse(
    sources: *const SourceView,
    count: usize,
) -> *mut CatalogReply {
    let result = (|| {
        if count > isize::MAX as usize / std::mem::size_of::<SourceView>()
            || (sources.is_null() && count != 0)
        {
            return Err(Fault::invalid("invalid source array"));
        }
        let input = if count == 0 {
            &[]
        } else {
            // SAFETY: size/null cases were checked; the caller supplies a live aligned array.
            unsafe { std::slice::from_raw_parts(sources, count) }
        };
        let mut owned = Vec::new();
        for source in input {
            // SAFETY: both source spans follow the public input contract.
            let path = PathBuf::from(OsStr::from_bytes(unsafe { bytes(source.path)? }));
            // SAFETY: the text span is borrowed under the same contract.
            let text = unsafe { text(source.text)? }.to_owned();
            owned.push(corpus::Source { path, text });
        }
        Ok(catalog(owned))
    })();
    Reply::finish(result)
}

/// Query mode 0 resolves a name, 1 searches terms, and 2 lists all entries.
/// The returned result owns its entries independently of the catalog.
/// # Safety
/// Handle must be live or null; query_text must be a readable UTF-8 span.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_query(
    handle: *const CatalogReply,
    mode: u32,
    query_text: Bytes,
) -> *mut MatchesReply {
    let result = (|| {
        // SAFETY: the caller supplies a live borrowed handle or null.
        let reply = unsafe { handle.as_ref() }.ok_or_else(|| Fault::invalid("null catalog"))?;
        let catalog = reply.0.as_ref().map_err(|error| Fault {
            code: error.code,
            message: error.message.clone(),
        })?;
        // SAFETY: query_text is borrowed for the duration of this call.
        let name = unsafe { text(query_text)? };
        query(&catalog.value, mode, name)
    })();
    Reply::finish(result)
}

fn query(catalog: &catalogue::Catalogue, mode: u32, name: &str) -> Result<Matches, Fault> {
    match mode {
        evman::QUERY_LOOKUP => {
            let found = catalog.lookup(name).ok_or_else(|| Fault {
                code: libc::ENOENT,
                message: format!("no catalog entry for {name}"),
            })?;
            Ok(lookup(found))
        }
        evman::QUERY_SEARCH => {
            let mut result = Matches::new(evman::MATCH_SEARCH);
            let terms: Vec<_> = name.split_whitespace().map(str::to_owned).collect();
            for entry in catalog.apropos(&terms) {
                result.add(entry);
            }
            Ok(result)
        }
        evman::QUERY_ALL => {
            let mut result = Matches::new(evman::MATCH_ALL);
            for entry in catalog
                .fields()
                .chain(catalog.groups())
                .chain(catalog.events())
            {
                result.add(entry);
            }
            Ok(result)
        }
        _ => Err(Fault::invalid("unknown catalog query mode")),
    }
}

fn lookup(found: catalogue::Lookup<'_>) -> Matches {
    match found {
        catalogue::Lookup::Event(entry) => {
            let mut result = Matches::new(evman::MATCH_EVENT);
            result.add(entry);
            result
        }
        catalogue::Lookup::Field(entry) => {
            let mut result = Matches::new(evman::MATCH_FIELD);
            result.add(entry);
            result
        }
        catalogue::Lookup::Variant { field, qualifier } => {
            let mut result = Matches::new(evman::MATCH_VARIANT);
            result.qualifier = qualifier.to_owned();
            result.add(field);
            result
        }
        catalogue::Lookup::Attribute { attribute, index } => {
            let mut result = Matches::new(evman::MATCH_ATTRIBUTE);
            result.attribute = attribute;
            result.index(index);
            result
        }
        catalogue::Lookup::Index(index) => {
            let mut result = Matches::new(evman::MATCH_INDEX);
            result.index(index);
            result
        }
    }
}

/// Release the catalog result and its borrowed views. Null is accepted.
/// # Safety
/// A non-null handle must be exclusively owned and freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_catalog_free(handle: *mut CatalogReply) {
    if !handle.is_null() {
        // SAFETY: the caller transfers this unique live allocation back to the library.
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Return zero for success, otherwise a POSIX error number. Null is EINVAL.
/// # Safety
/// A non-null handle must be a live borrowed catalog result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_catalog_error(handle: *const CatalogReply) -> i32 {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }.map_or(libc::EINVAL, Reply::code)
}

/// Borrow the failure diagnostic; success and null return an absent span.
/// # Safety
/// A non-null handle must be a live borrowed catalog result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_catalog_message(handle: *const CatalogReply) -> Bytes {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }.map_or_else(Bytes::absent, Reply::message)
}

/// Release the matches result and its borrowed views. Null is accepted.
/// # Safety
/// A non-null handle must be exclusively owned and freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_matches_free(handle: *mut MatchesReply) {
    if !handle.is_null() {
        // SAFETY: the caller transfers this unique live allocation back to the library.
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Return zero for success, otherwise a POSIX error number. Null is EINVAL.
/// # Safety
/// A non-null handle must be a live borrowed matches result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_matches_error(handle: *const MatchesReply) -> i32 {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }.map_or(libc::EINVAL, Reply::code)
}

/// Borrow the failure diagnostic; success and null return an absent span.
/// # Safety
/// A non-null handle must be a live borrowed matches result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_matches_message(handle: *const MatchesReply) -> Bytes {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }.map_or_else(Bytes::absent, Reply::message)
}

/// Return the finding count, or zero for a failed or null result.
/// # Safety
/// A non-null handle must be a live borrowed result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_finding_count(handle: *const CatalogReply) -> usize {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }
        .and_then(|reply| reply.0.as_ref().ok())
        .map_or(0, |value| value.findings.len())
}

/// Return the record count, or zero for a failed or null result.
/// # Safety
/// A non-null handle must be a live borrowed result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_record_count(handle: *const MatchesReply) -> usize {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }
        .and_then(|reply| reply.0.as_ref().ok())
        .map_or(0, |value| value.records.len())
}

/// Borrow the lookup qualifier; unrelated query kinds return an empty span.
/// # Safety
/// A non-null handle must be a live borrowed query result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_qualifier(handle: *const MatchesReply) -> Bytes {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }
        .and_then(|reply| reply.0.as_ref().ok())
        .map_or_else(Bytes::absent, |value| Bytes::text(&value.qualifier))
}

/// Borrow the lookup attribute; unrelated query kinds return an empty span.
/// # Safety
/// A non-null handle must be a live borrowed query result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_attribute(handle: *const MatchesReply) -> Bytes {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }
        .and_then(|reply| reply.0.as_ref().ok())
        .map_or_else(Bytes::absent, |value| Bytes::text(&value.attribute))
}

/// Borrow the lookup prefix; unrelated query kinds return an empty span.
/// # Safety
/// A non-null handle must be a live borrowed query result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_prefix(handle: *const MatchesReply) -> Bytes {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }
        .and_then(|reply| reply.0.as_ref().ok())
        .map_or_else(Bytes::absent, |value| Bytes::text(&value.prefix))
}

/// Return the lookup kind (1 through 7), or zero for null/failure.
/// # Safety
/// A non-null handle must be a live borrowed query result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_match_kind(handle: *const MatchesReply) -> u32 {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }
        .and_then(|reply| reply.0.as_ref().ok())
        .map_or(evman::MATCH_INVALID, |value| value.kind)
}

/// Borrow a query record, reporting invalid handles and indices.
///
/// # Safety
/// A non-null pointer must identify a live matches reply from this library.
/// Its allocation must remain alive and immutable for the entire returned
/// lifetime, not merely until this function returns.
unsafe fn record<'a>(handle: *const MatchesReply, index: usize) -> Result<&'a Record, i32> {
    // SAFETY: internal callers forward a live borrowed handle or null.
    let reply = unsafe { handle.as_ref() }.ok_or(libc::EINVAL)?;
    let matches = reply.0.as_ref().map_err(|error| error.code)?;
    matches.records.get(index).ok_or(libc::ERANGE)
}

/// Copy a finding view. Failure leaves output untouched and returns a POSIX error number.
/// # Safety
/// Handle must be live or null. Output must be aligned and writable, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_finding(
    handle: *const CatalogReply,
    index: usize,
    output: *mut FindingView,
) -> i32 {
    if output.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: the caller supplies a live borrowed handle or null.
    let Some(reply) = (unsafe { handle.as_ref() }) else {
        return libc::EINVAL;
    };
    let catalog = match &reply.0 {
        Ok(value) => value,
        Err(error) => return error.code,
    };
    let Some(finding) = catalog.findings.get(index) else {
        return libc::ERANGE;
    };
    let rule = match finding.rule {
        lint::Rule::Format => evman::RULE_FORMAT,
        lint::Rule::Numbered(n) => u32::from(n),
    };
    // SAFETY: output is a valid writable aligned slot, initialized without reading it.
    unsafe {
        output.write(FindingView {
            file: Bytes::new(finding.file.as_os_str().as_bytes()),
            line: finding.line as u64,
            rule,
            message: Bytes::text(&finding.message),
        })
    };
    0
}

/// Copy a record view. Kinds are field=1, group=2, event=3.
/// # Safety
/// Handle must be live or null. Output must be aligned and writable, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_record(
    handle: *const MatchesReply,
    index: usize,
    output: *mut RecordView,
) -> i32 {
    if output.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: the caller supplies a live borrowed handle or null.
    let record = match unsafe { record(handle, index) } {
        Ok(value) => value,
        Err(code) => return code,
    };
    let entry = &record.entry;
    let kind = match entry.record.kind {
        fragment::Kind::Field => evman::RECORD_FIELD,
        fragment::Kind::Group => evman::RECORD_GROUP,
        fragment::Kind::Event => evman::RECORD_EVENT,
    };
    // SAFETY: output is a valid writable aligned slot, initialized without reading it.
    unsafe {
        output.write(RecordView {
            kind,
            name: Bytes::text(&entry.record.name),
            file: Bytes::new(entry.file.as_os_str().as_bytes()),
            provider: Bytes::text(&entry.fragment),
            body: Bytes::text(&record.body),
            line: entry.record.line as u64,
        })
    };
    0
}

/// Return a record's header count, or zero for invalid inputs.
/// # Safety
/// A non-null handle must be a live borrowed query result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_header_count(
    handle: *const MatchesReply,
    index: usize,
) -> usize {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { record(handle, index) }.map_or(0, |value| value.entry.record.headers.len())
}

/// Return a record's include count, or zero for invalid inputs.
/// # Safety
/// A non-null handle must be a live borrowed query result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_include_count(
    handle: *const MatchesReply,
    index: usize,
) -> usize {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { record(handle, index) }.map_or(0, |value| value.entry.record.includes.len())
}

/// Return a record's line count, or zero for invalid inputs.
/// # Safety
/// A non-null handle must be a live borrowed query result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_line_count(
    handle: *const MatchesReply,
    index: usize,
) -> usize {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { record(handle, index) }.map_or(0, |value| value.entry.record.lines.len())
}

/// Copy a header view. Failure returns a POSIX error number and leaves output untouched.
/// # Safety
/// Handle must be live or null. Output must be aligned and writable, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_header(
    handle: *const MatchesReply,
    record_index: usize,
    index: usize,
    output: *mut HeaderView,
) -> i32 {
    if output.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: the caller supplies a live borrowed handle or null.
    let record = match unsafe { record(handle, record_index) } {
        Ok(value) => value,
        Err(code) => return code,
    };
    let Some(item) = record.entry.record.headers.get(index) else {
        return libc::ERANGE;
    };
    // SAFETY: output is a valid writable aligned slot, initialized without reading it.
    unsafe {
        output.write(HeaderView {
            key: Bytes::text(&item.key),
            value: Bytes::text(&item.value),
            line: item.line as u64,
        })
    };
    0
}

/// Copy a include view. Failure returns a POSIX error number and leaves output untouched.
/// # Safety
/// Handle must be live or null. Output must be aligned and writable, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_include(
    handle: *const MatchesReply,
    record_index: usize,
    index: usize,
    output: *mut LineView,
) -> i32 {
    if output.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: the caller supplies a live borrowed handle or null.
    let record = match unsafe { record(handle, record_index) } {
        Ok(value) => value,
        Err(code) => return code,
    };
    let Some(item) = record.entry.record.includes.get(index) else {
        return libc::ERANGE;
    };
    // SAFETY: output is a valid writable aligned slot, initialized without reading it.
    unsafe {
        output.write(LineView {
            kind: evman::LINE_INCLUDE,
            name: Bytes::text(&item.0),
            presence: Bytes::absent(),
            values: Bytes::absent(),
            closed: Bytes::absent(),
            line: item.1 as u64,
            gloss: Bytes::absent(),
        })
    };
    0
}

/// Copy a line view. Failure returns a POSIX error number and leaves output untouched.
/// # Safety
/// Handle must be live or null. Output must be aligned and writable, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_evman_line(
    handle: *const MatchesReply,
    record_index: usize,
    index: usize,
    output: *mut LineView,
) -> i32 {
    if output.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: the caller supplies a live borrowed handle or null.
    let record = match unsafe { record(handle, record_index) } {
        Ok(value) => value,
        Err(code) => return code,
    };
    let Some(item) = record.entry.record.lines.get(index) else {
        return libc::ERANGE;
    };
    // SAFETY: output is a valid writable aligned slot, initialized without reading it.
    unsafe {
        output.write(LineView {
            kind: match item.kind {
                fragment::LineKind::Field => evman::LINE_FIELD,
                fragment::LineKind::Include => evman::LINE_INCLUDE,
            },
            name: Bytes::text(&item.name),
            presence: Bytes::text(&item.presence),
            values: Bytes::optional(item.values.as_ref().map(|(value, _)| value.as_str())),
            closed: Bytes::optional(item.closed.as_ref().map(|(value, _)| value.as_str())),
            line: item.line as u64,
            gloss: Bytes::text(&record.line_glosses[index]),
        })
    };
    0
}
