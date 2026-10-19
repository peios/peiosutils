//! C interface for registry batch documents. See include/peios/reg.h.
//!
//! Non-null inputs must be valid for their stated lengths. Handles must come
//! from this library. Returned spans borrow their handle, remain immutable,
//! and expire when it is freed. Free requires exclusive ownership.

use utility_ffi::{Bytes, Fault, Reply, text};

/// Immutable document and its JSON representation.
pub struct Document {
    value: registry_docs::Document,
    json: Vec<u8>,
}

pub type DocumentReply = Reply<Document>;
pub type ApplyReply = Reply<usize>;

fn fault(error: registry_docs::Error) -> Fault {
    let code = match &error {
        registry_docs::Error::Registry { source, .. } => source.raw_os_error().unwrap_or(libc::EIO),
        registry_docs::Error::Invalid(_) => libc::EINVAL,
    };
    Fault {
        code,
        message: error.to_string(),
    }
}

fn document(value: registry_docs::Document) -> Result<Document, Fault> {
    let json = serde_json::to_vec(&value).map_err(|error| Fault::invalid(error.to_string()))?;
    Ok(Document { value, json })
}

/// Return the C ABI major version.
#[unsafe(no_mangle)]
pub extern "C" fn peios_reg_abi_version() -> u32 {
    1
}

/// Parse a registry batch document. The input is copied and need not outlive the call.
/// # Safety
/// The JSON span must be readable for its stated length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_reg_document_parse(json: Bytes) -> *mut DocumentReply {
    // SAFETY: json is borrowed under this function's input contract.
    let parsed = unsafe { text(json) }
        .and_then(|text| {
            serde_json::from_str(text).map_err(|error| Fault::invalid(error.to_string()))
        })
        .and_then(document);
    Reply::finish(parsed)
}

/// Export the live registry subtree at a UTF-8 path, preserving registry errors.
/// # Safety
/// The path span must be readable for its stated length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_reg_document_export(path: Bytes) -> *mut DocumentReply {
    // SAFETY: path is borrowed under this function's input contract.
    Reply::finish(
        unsafe { text(path) }
            .and_then(|path| registry_docs::export(path).map_err(fault))
            .and_then(document),
    )
}

/// Apply a document in one transaction. A null layer means the existing default layer.
/// # Safety
/// The document must be a live borrowed handle or null; layer must be a valid span.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_reg_document_apply(
    handle: *const DocumentReply,
    layer: Bytes,
) -> *mut ApplyReply {
    let result = (|| {
        // SAFETY: the caller borrows a live handle or passes null.
        let reply = unsafe { handle.as_ref() }.ok_or_else(|| Fault::invalid("null document"))?;
        let value = reply.0.as_ref().map_err(|error| Fault {
            code: error.code,
            message: error.message.clone(),
        })?;
        // SAFETY: layer follows the span input contract, even when absent.
        let layer_text = unsafe { text(layer)? };
        let selected = (!layer.data.is_null()).then_some(layer_text);
        registry_docs::apply(&value.value, selected).map_err(fault)
    })();
    Reply::finish(result)
}

/// Borrow the document's JSON, or an absent span for a failed or null handle.
/// # Safety
/// A non-null handle must be a live borrowed document reply.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_reg_document_json(handle: *const DocumentReply) -> Bytes {
    // SAFETY: handle validity is required by the caller contract.
    unsafe { handle.as_ref() }
        .and_then(|reply| reply.0.as_ref().ok())
        .map_or_else(Bytes::absent, |document| Bytes::new(&document.json))
}

/// Return the number of keys applied, or zero for a failed or null handle.
/// # Safety
/// A non-null handle must be a live borrowed apply reply.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_reg_apply_count(handle: *const ApplyReply) -> usize {
    // SAFETY: handle validity is required by the caller contract.
    unsafe { handle.as_ref() }
        .and_then(|reply| reply.0.as_ref().ok())
        .copied()
        .unwrap_or(0)
}

/// Release a document reply. Null is accepted.
/// # Safety
/// A non-null handle must be exclusively owned and freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_reg_document_free(handle: *mut DocumentReply) {
    if !handle.is_null() {
        // SAFETY: the caller transfers its unique live allocation back to this library.
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Return zero on success, otherwise a POSIX error number. Null is EINVAL.
/// # Safety
/// A non-null handle must be a live borrowed document reply.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_reg_document_error(handle: *const DocumentReply) -> i32 {
    // SAFETY: the caller supplies a live borrowed reply or null.
    unsafe { handle.as_ref() }.map_or(libc::EINVAL, Reply::code)
}

/// Borrow diagnostic text from a failed reply; success or null yields absence.
/// # Safety
/// A non-null handle must be a live borrowed document reply.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_reg_document_message(handle: *const DocumentReply) -> Bytes {
    // SAFETY: the caller supplies a live borrowed reply or null.
    unsafe { handle.as_ref() }.map_or_else(Bytes::absent, Reply::message)
}

/// Release a apply reply. Null is accepted.
/// # Safety
/// A non-null handle must be exclusively owned and freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_reg_apply_free(handle: *mut ApplyReply) {
    if !handle.is_null() {
        // SAFETY: the caller transfers its unique live allocation back to this library.
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Return zero on success, otherwise a POSIX error number. Null is EINVAL.
/// # Safety
/// A non-null handle must be a live borrowed apply reply.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_reg_apply_error(handle: *const ApplyReply) -> i32 {
    // SAFETY: the caller supplies a live borrowed reply or null.
    unsafe { handle.as_ref() }.map_or(libc::EINVAL, Reply::code)
}

/// Borrow diagnostic text from a failed reply; success or null yields absence.
/// # Safety
/// A non-null handle must be a live borrowed apply reply.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn peios_reg_apply_message(handle: *const ApplyReply) -> Bytes {
    // SAFETY: the caller supplies a live borrowed reply or null.
    unsafe { handle.as_ref() }.map_or_else(Bytes::absent, Reply::message)
}
