//! Internal ownership helpers for immutable utility-library replies.

use std::ffi::c_void;
use std::fmt;
use std::ptr::NonNull;

use utility_ffi::Bytes;

/// A failure reported by a utility shared library.
#[derive(Debug, Clone)]
pub struct Error {
    /// Nonzero POSIX error number, independent of thread-local errno.
    pub code: i32,
    /// Owned diagnostic text supplied by the library or local errno description.
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for Error {}

impl Error {
    /// Describe an accessor failure using its POSIX error number.
    pub fn from_code(code: i32) -> Self {
        Self {
            code,
            message: std::io::Error::from_raw_os_error(code).to_string(),
        }
    }
}

/// Unique destruction ownership of an immutable shared-library reply.
///
/// The matching accessors may borrow the pointer while this handle remains alive.
pub struct Handle {
    raw: NonNull<c_void>,
    free: unsafe extern "C" fn(*mut c_void),
}

impl Handle {
    /// Adopt a result with its library's matching status, message, and free functions.
    /// # Safety
    /// The pointer and functions must belong to the same live immutable reply type.
    /// Ownership of its sole destruction right transfers into this call, including
    /// when the function returns an error. Its contract must permit concurrent
    /// reads and destruction on any thread.
    pub unsafe fn adopt(
        raw: *mut c_void,
        free: unsafe extern "C" fn(*mut c_void),
        error: unsafe extern "C" fn(*const c_void) -> i32,
        message: unsafe extern "C" fn(*const c_void) -> Bytes,
    ) -> Result<Self, Error> {
        let raw = NonNull::new(raw).ok_or_else(|| Error {
            code: libc::ENOMEM,
            message: "shared library returned a null reply".to_owned(),
        })?;
        let handle = Self { raw, free };
        // SAFETY: the pointer and accessor belong to this immutable live reply.
        let code = unsafe { error(handle.as_ptr()) };
        if code != 0 {
            // SAFETY: the accessor returns a readable span borrowed from this live reply.
            let message = unsafe { copy_text(message(handle.as_ptr())) };
            return Err(Error { code, message });
        }
        Ok(handle)
    }

    /// Borrow the underlying reply; the pointer must not outlive this owner.
    pub fn as_ptr(&self) -> *const c_void {
        self.raw.as_ptr()
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns the only destruction right for its live reply.
        unsafe { (self.free)(self.raw.as_ptr()) };
    }
}

// SAFETY: all accepted handles are immutable and their documented APIs permit
// concurrent reads. Rust ownership prevents destruction while borrowed.
unsafe impl Send for Handle {}
// SAFETY: the same immutable-handle contract permits shared concurrent access.
unsafe impl Sync for Handle {}

/// Copy bytes borrowed from a live result.
/// # Safety
/// The span must be a valid library-produced view for the duration of this call.
pub unsafe fn copy_bytes(value: Bytes) -> Vec<u8> {
    if value.len == 0 {
        return Vec::new();
    }
    // SAFETY: validity and allocation bounds follow from the borrowed-view contract.
    unsafe { std::slice::from_raw_parts(value.data, value.len) }.to_vec()
}

/// Copy a UTF-8 view borrowed from a live result.
/// # Safety
/// The span must be a valid library-produced UTF-8 view for this call.
pub unsafe fn copy_text(value: Bytes) -> String {
    // SAFETY: the input is a live library-produced span.
    String::from_utf8(unsafe { copy_bytes(value) }).expect("utility library returned invalid UTF-8")
}

/// Copy an optional UTF-8 field, preserving absent versus present empty values.
/// # Safety
/// The span must be a valid library-produced optional UTF-8 view for this call.
pub unsafe fn optional_text(value: Bytes) -> Option<String> {
    if value.data.is_null() {
        None
    } else {
        // SAFETY: a present field follows the documented UTF-8 view contract.
        Some(unsafe { copy_text(value) })
    }
}

/// Read a view through an indexed C getter.
/// # Safety
/// On success (code zero), the callback must have initialized exactly one valid T.
pub unsafe fn view<T>(read: impl FnOnce(*mut T) -> i32) -> Result<T, Error> {
    let mut output = std::mem::MaybeUninit::uninit();
    let code = read(output.as_mut_ptr());
    if code != 0 {
        return Err(Error::from_code(code));
    }
    // SAFETY: the callback's success contract guarantees complete initialization.
    Ok(unsafe { output.assume_init() })
}
