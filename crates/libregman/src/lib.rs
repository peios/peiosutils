//! C interface for registry manuals. See include/peios/regman.h.
//!
//! Inputs are borrowed during a call. Non-null spans must be readable for
//! their lengths. Handles belong to this library. Output views remain valid
//! until their immutable result is freed; freeing requires exclusive access.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use registry_manual::{corpus, fold, fragment, query, scan};
use utility_ffi::{Bytes, Fault, Reply, bytes, text, regman};

/// Parsed or resolved records and recoverable fragment diagnostics.
pub struct Records {
    hits: Vec<scan::Hit>,
    issues: Vec<fragment::ParseIssue>,
}

pub type RecordsReply = Reply<Records>;

/// Borrowed fields of one registry-manual record.
pub use utility_ffi::regman::{RecordView, IssueView};

fn fault(error: registry_manual::error::Error) -> Fault {
    let code = match &error {
        registry_manual::error::Error::NotFound(_) => libc::ENOENT,
        registry_manual::error::Error::Io(_) => libc::EIO,
        _ => libc::EINVAL,
    };
    Fault {
        code,
        message: error.to_string(),
    }
}

/// Copy an explicit Unix path or obtain the default for an absent span.
///
/// # Safety
/// The input must satisfy the readable allocation and lifetime contract of
/// [`bytes`] for the duration of this call.
unsafe fn path(value: Bytes, default: impl FnOnce() -> PathBuf) -> Result<PathBuf, Fault> {
    // SAFETY: exported callers forward a valid borrowed span.
    let value_bytes = unsafe { bytes(value)? };
    Ok(if value.data.is_null() {
        default()
    } else {
        PathBuf::from(OsStr::from_bytes(value_bytes))
    })
}

/// Return the C ABI major version.
#[unsafe(no_mangle)]
pub extern "C" fn peios_regman_abi_version() -> u32 {
    1
}

/// Parse a fragment, returning valid records and diagnostics together.
/// # Safety
/// The UTF-8 source span must be readable for its stated length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_regman_parse(source: Bytes) -> *mut RecordsReply {
    // SAFETY: source is borrowed under the public input contract.
    Reply::finish(unsafe { text(source) }.map(|source| {
        let (records, issues) = fragment::parse(source);
        let hits = records
            .into_iter()
            .map(|record| scan::Hit {
                provider: String::new(),
                file: PathBuf::new(),
                record,
            })
            .collect();
        Records { hits, issues }
    }))
}

/// Query a manual: mode 0 is exact, 1 is key, and 2 searches whitespace-separated terms.
/// Input text is case-folded by the library. Absent paths select corpus defaults.
/// # Safety
/// All spans must be readable for their stated lengths. Paths contain raw Unix bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_regman_query(
    directory: Bytes,
    index: Bytes,
    mode: u32,
    query_text: Bytes,
) -> *mut RecordsReply {
    let result = (|| {
        // SAFETY: the exported input contract covers each borrowed span.
        let directory = unsafe { path(directory, corpus::dir)? };
        // SAFETY: index follows the same input contract.
        let index = unsafe { path(index, corpus::index_path)? };
        // SAFETY: query_text follows the same input contract and must be UTF-8.
        let query_text = fold::fold(unsafe { text(query_text)? });
        let hits = match mode {
            regman::QUERY_EXACT => query::resolve_exact(&directory, &index, &query_text),
            regman::QUERY_KEY => query::resolve_key(&directory, &index, &query_text),
            regman::QUERY_SEARCH => scan::apropos(
                &directory,
                &query_text
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
            ),
            _ => return Err(Fault::invalid("unknown manual query mode")),
        }
        .map_err(fault)?;
        Ok(Records {
            hits,
            issues: Vec::new(),
        })
    })();
    Reply::finish(result)
}

/// Release a result and every view it owns. Null is accepted.
/// # Safety
/// A non-null handle must be exclusively owned and freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_regman_free(handle: *mut RecordsReply) {
    if !handle.is_null() {
        // SAFETY: ownership of this live allocation transfers back to the library.
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Return zero on success or a POSIX error number. Null is EINVAL.
/// # Safety
/// A non-null handle must be a live borrowed result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_regman_error(handle: *const RecordsReply) -> i32 {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }.map_or(libc::EINVAL, Reply::code)
}

/// Borrow a diagnostic message; success and null return an absent span.
/// # Safety
/// A non-null handle must be a live borrowed result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_regman_message(handle: *const RecordsReply) -> Bytes {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }.map_or_else(Bytes::absent, Reply::message)
}

/// Return the record count, or zero on failure or null.
/// # Safety
/// A non-null handle must be a live borrowed result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_regman_record_count(handle: *const RecordsReply) -> usize {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }
        .and_then(|reply| reply.0.as_ref().ok())
        .map_or(0, |records| records.hits.len())
}

/// Return the issue count, or zero on failure or null.
/// # Safety
/// A non-null handle must be a live borrowed result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_regman_issue_count(handle: *const RecordsReply) -> usize {
    // SAFETY: the caller supplies a live borrowed handle or null.
    unsafe { handle.as_ref() }
        .and_then(|reply| reply.0.as_ref().ok())
        .map_or(0, |records| records.issues.len())
}

/// Copy a record view to output; return EINVAL or ERANGE without writing on failure.
/// # Safety
/// The handle must be live or null. Output must be writable and aligned, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_regman_record(
    handle: *const RecordsReply,
    index: usize,
    output: *mut RecordView,
) -> i32 {
    if output.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: the caller supplies a live borrowed handle or null.
    let Some(reply) = (unsafe { handle.as_ref() }) else {
        return libc::EINVAL;
    };
    let records = match &reply.0 {
        Ok(records) => records,
        Err(error) => return error.code,
    };
    let Some(hit) = records.hits.get(index) else {
        return libc::ERANGE;
    };
    let record = &hit.record;
    let view = RecordView {
        provider: Bytes::text(&hit.provider),
        file: Bytes::new(hit.file.as_os_str().as_bytes()),
        anchor: Bytes::text(&record.anchor),
        canonical: Bytes::text(&record.canonical),
        type_name: Bytes::optional(record.type_.as_deref()),
        default_value: Bytes::optional(record.default.as_deref()),
        valid: Bytes::optional(record.valid.as_deref()),
        applies: Bytes::optional(record.applies.as_deref()),
        deprecated: Bytes::optional(record.deprecated.as_deref()),
        body: Bytes::text(&record.body),
        fence_offset: record.fence_offset as u64,
    };
    // SAFETY: output is a writable aligned slot; it is initialized without reading old bytes.
    unsafe { output.write(view) };
    0
}

/// Copy a parser diagnostic; return EINVAL or ERANGE without writing on failure.
/// # Safety
/// The handle must be live or null. Output must be writable and aligned, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_regman_issue(
    handle: *const RecordsReply,
    index: usize,
    output: *mut IssueView,
) -> i32 {
    if output.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: the caller supplies a live borrowed handle or null.
    let Some(reply) = (unsafe { handle.as_ref() }) else {
        return libc::EINVAL;
    };
    let records = match &reply.0 {
        Ok(records) => records,
        Err(error) => return error.code,
    };
    let Some(issue) = records.issues.get(index) else {
        return libc::ERANGE;
    };
    // SAFETY: output is a writable aligned slot, initialized without reading old bytes.
    unsafe {
        output.write(IssueView {
            line: issue.line as u64,
            message: Bytes::text(&issue.message),
        })
    };
    0
}
