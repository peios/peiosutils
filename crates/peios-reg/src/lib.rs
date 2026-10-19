//! Registry batch documents backed by libreg.so.
//!
//! Install the matching shared library, or set PEIOS_REG_LIB_DIR when building
//! with Cargo. Documents own immutable library handles and release them on drop.

use std::ffi::c_void;

use utility_client::Handle;
use utility_ffi::Bytes;

pub use utility_client::Error;

#[link(name = "reg")]
unsafe extern "C" {
    fn peios_reg_document_parse(json: Bytes) -> *mut c_void;
    fn peios_reg_document_export(path: Bytes) -> *mut c_void;
    fn peios_reg_document_apply(handle: *const c_void, layer: Bytes) -> *mut c_void;
    fn peios_reg_document_json(handle: *const c_void) -> Bytes;
    fn peios_reg_document_free(handle: *mut c_void);
    fn peios_reg_document_error(handle: *const c_void) -> i32;
    fn peios_reg_document_message(handle: *const c_void) -> Bytes;
    fn peios_reg_apply_free(handle: *mut c_void);
    fn peios_reg_apply_error(handle: *const c_void) -> i32;
    fn peios_reg_apply_message(handle: *const c_void) -> Bytes;
    fn peios_reg_apply_count(handle: *const c_void) -> usize;
}

/// An immutable registry batch document, owned by libreg.
pub struct Document(Handle);

impl Document {
    /// Take ownership of a document reply, releasing failures automatically.
    ///
    /// # Safety
    /// A non-null pointer must be a live libreg document reply. Its unique
    /// destruction right transfers to this call, including when an error is returned.
    unsafe fn adopt(raw: *mut c_void) -> Result<Self, Error> {
        // SAFETY: internal callers pass a new document reply from libreg.
        unsafe {
            Handle::adopt(
                raw,
                peios_reg_document_free,
                peios_reg_document_error,
                peios_reg_document_message,
            )
        }
        .map(Self)
    }

    /// Parse JSON without changing the registry.
    ///
    /// Malformed JSON or a mismatched document shape returns EINVAL. Parsing copies
    /// the input. Value, path, and descriptor validation also occurs on apply.
    pub fn parse(json: &str) -> Result<Self, Error> {
        // SAFETY: the input is borrowed for the call; the returned reply transfers ownership.
        unsafe { Self::adopt(peios_reg_document_parse(Bytes::text(json))) }
    }

    /// Read a registry subtree using the caller's existing access token.
    ///
    /// Keys are returned depth first, parents before children. Values that cannot
    /// be read are omitted. Opening keys or enumerating children can return a
    /// registry error. Export does not read or include security descriptors.
    pub fn export(path: &str) -> Result<Self, Error> {
        // SAFETY: path is borrowed for the call; the returned reply transfers ownership.
        unsafe { Self::adopt(peios_reg_document_export(Bytes::text(path))) }
    }

    /// Borrow the document's UTF-8 JSON representation until this document is dropped.
    pub fn json(&self) -> &str {
        // SAFETY: the immutable result owns the returned bytes for this borrow's lifetime.
        let value = unsafe { peios_reg_document_json(self.0.as_ptr()) };
        // SAFETY: a successful parsed/exported document owns a nonempty JSON buffer.
        let bytes = unsafe { std::slice::from_raw_parts(value.data, value.len) };
        std::str::from_utf8(bytes).expect("libreg returned invalid JSON encoding")
    }

    /// Apply all keys in one transaction; `None` selects the registry's default layer.
    ///
    /// The result counts keys, not values. Invalid document content returns EINVAL;
    /// registry failures preserve their POSIX error code. Permissions come from the
    /// caller's existing access token. A failed transaction is not committed.
    pub fn apply(&self, layer: Option<&str>) -> Result<usize, Error> {
        // SAFETY: this live immutable document and the optional layer are borrowed for the call.
        let raw = unsafe { peios_reg_document_apply(self.0.as_ptr(), Bytes::optional(layer)) };
        // SAFETY: raw is a new apply reply from the matching library.
        let result = unsafe {
            Handle::adopt(
                raw,
                peios_reg_apply_free,
                peios_reg_apply_error,
                peios_reg_apply_message,
            )
        }?;
        // SAFETY: result is a successful live apply reply, retained through this call.
        Ok(unsafe { peios_reg_apply_count(result.as_ptr()) })
    }
}

#[cfg(test)]
mod tests {
    use super::Document;

    #[test]
    fn document_outlives_its_input() {
        let document = Document::parse(&String::from("{\"keys\":[]}")).unwrap();
        assert_eq!(document.json(), "{\"keys\":[]}");
    }

    #[test]
    fn malformed_json_is_a_normal_error() {
        assert!(Document::parse("{").is_err());
    }
}
