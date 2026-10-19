//! Internal helpers shared by the utility C interfaces.

use std::ptr;
use std::slice;

/// A borrowed byte span. Null with zero length denotes absence.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Bytes {
    /// Borrowed allocation address, or null when the span is absent.
    pub data: *const u8,
    /// Number of bytes in the span; zero for absence.
    pub len: usize,
}

impl Bytes {
    /// Construct the absent `{NULL, 0}` representation.
    pub const fn absent() -> Self {
        Self {
            data: ptr::null(),
            len: 0,
        }
    }

    /// Borrow bytes without extending their lifetime. The span must not outlive the slice.
    pub fn new(value: &[u8]) -> Self {
        Self {
            data: value.as_ptr(),
            len: value.len(),
        }
    }

    /// Borrow UTF-8 text without extending its lifetime.
    pub fn text(value: &str) -> Self {
        Self::new(value.as_bytes())
    }

    /// Borrow optional UTF-8 text, preserving absence versus an empty string.
    pub fn optional(value: Option<&str>) -> Self {
        value.map_or_else(Self::absent, Self::text)
    }
}

/// An owned diagnostic with a POSIX error number.
#[derive(Debug)]
pub struct Fault {
    /// Nonzero POSIX error number, independent of thread-local errno.
    pub code: i32,
    /// Owned UTF-8 description of the failure.
    pub message: String,
}

impl Fault {
    /// Describe invalid caller input using EINVAL.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: libc::EINVAL,
            message: message.into(),
        }
    }
}

/// Every operation returns an owned reply, including failures.
pub struct Reply<T>(
    /// The successful immutable value or its owned failure diagnostic.
    pub Result<T, Fault>,
);

impl<T> Reply<T> {
    /// Transfer a newly allocated reply to its matching library destructor.
    pub fn finish(result: Result<T, Fault>) -> *mut Self {
        Box::into_raw(Box::new(Self(result)))
    }

    /// Return zero for success or the stored POSIX error number.
    pub fn code(&self) -> i32 {
        self.0.as_ref().err().map_or(0, |error| error.code)
    }

    /// Borrow the failure diagnostic; success returns an absent span.
    pub fn message(&self) -> Bytes {
        self.0
            .as_ref()
            .err()
            .map_or_else(Bytes::absent, |error| Bytes::text(&error.message))
    }
}

/// Borrow an input span for a call. Null is accepted only with zero length.
/// # Safety
/// A non-null pointer must identify initialized, readable bytes for the stated
/// length in one live allocation, borrowed for the returned lifetime.
pub unsafe fn bytes<'a>(value: Bytes) -> Result<&'a [u8], Fault> {
    if value.len > isize::MAX as usize || (value.data.is_null() && value.len != 0) {
        return Err(Fault::invalid("invalid byte span"));
    }
    if value.len == 0 {
        return Ok(&[]);
    }
    // SAFETY: length and null cases were checked; allocation validity is the caller's contract.
    Ok(unsafe { slice::from_raw_parts(value.data, value.len) })
}

/// Borrow a UTF-8 input under the same allocation contract as [`bytes`].
/// # Safety
/// The input must satisfy [`bytes`]'s pointer and lifetime contract.
pub unsafe fn text<'a>(value: Bytes) -> Result<&'a str, Fault> {
    // SAFETY: the caller supplies a borrowed input span.
    std::str::from_utf8(unsafe { bytes(value)? }).map_err(|_| Fault::invalid("invalid UTF-8"))
}

pub mod evman;
pub mod regman;
