// Error type for pu_sd. Mirrors pu_token's shape.

use peios::Error as PeiosError;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Bad flags, conflicting options, malformed input. Exit 1.
    #[error("usage: {0}")]
    Usage(String),

    /// No such path, principal, or class. Exit 2.
    #[error("not found: {0}")]
    NotFound(String),

    /// KACS denied the operation. Exit 3.
    #[error("permission denied: {what} (errno {errno})")]
    Denied { what: String, errno: i32 },

    /// SD / SDDL / spec didn't parse or wouldn't encode. Exit 4.
    #[error("invalid: {0}")]
    Invalid(String),

    /// Any other syscall error. Exit 5.
    #[error("{op}: {errno}")]
    Syscall { op: String, errno: ErrnoDisplay },
}

#[derive(Debug)]
pub struct ErrnoDisplay(pub i32);

impl fmt::Display for ErrnoDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", std::io::Error::from_raw_os_error(self.0))
    }
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) => 1,
            Error::NotFound(_) => 2,
            Error::Denied { .. } => 3,
            Error::Invalid(_) => 4,
            Error::Syscall { .. } => 5,
        }
    }
}

impl From<PeiosError> for Error {
    fn from(e: PeiosError) -> Self {
        // peios::Error is an io::Error-like wrapper over `errno`; the raw value
        // is always present for an OS error. EACCES / EPERM map to a denial,
        // ENOENT to NotFound, everything else to a generic syscall error.
        let raw = e.raw_os_error().unwrap_or(0);
        if raw == libc::EACCES || raw == libc::EPERM {
            Error::Denied {
                what: "kacs SD syscall".to_string(),
                errno: raw,
            }
        } else if raw == libc::ENOENT {
            Error::NotFound(format!("kacs SD syscall: {e}"))
        } else {
            Error::Syscall {
                op: "kacs SD syscall".to_string(),
                errno: ErrnoDisplay(raw),
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
