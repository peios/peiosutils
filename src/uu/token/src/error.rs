// token's error type. Maps cleanly to the exit-code table in the
// design doc (peios/token-design.md §"Error model").

use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// Usage / argument problem (exit 1).
    Usage(String),
    /// No such target / class (exit 2).
    NotFound(String),
    /// KACS denied the call (exit 3). Carries the access mask that
    /// the caller would have needed, if known.
    Denied {
        op: &'static str,
        target: String,
        needed_mask: Option<u32>,
        errno: i32,
    },
    /// Invalid spec input (exit 4) — failed to parse/encode JSON.
    InvalidSpec(String),
    /// Other syscall error (exit 5).
    Syscall {
        op: &'static str,
        errno: i32,
        detail: Option<String>,
    },
    /// Failed to decode a query payload returned by KACS.
    Decode(String),
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) => 1,
            Error::NotFound(_) => 2,
            Error::Denied { .. } => 3,
            Error::InvalidSpec(_) => 4,
            Error::Syscall { .. } | Error::Decode(_) => 5,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(m) => write!(f, "{m}"),
            Error::NotFound(m) => write!(f, "not found: {m}"),
            Error::Denied {
                op,
                target,
                needed_mask,
                errno,
            } => {
                let e = peios::Error::from_raw_os_error(*errno);
                write!(f, "{op}: {e}\n  target: {target}")?;
                if let Some(mask) = needed_mask {
                    write!(
                        f,
                        "\n  needed access: {}",
                        crate::render::access_mask_names(*mask)
                    )?;
                }
                Ok(())
            }
            Error::InvalidSpec(m) => write!(f, "invalid spec: {m}"),
            Error::Syscall { op, errno, detail } => {
                let e = peios::Error::from_raw_os_error(*errno);
                write!(f, "{op}: {e}")?;
                if let Some(d) = detail {
                    write!(f, " ({d})")?;
                }
                Ok(())
            }
            Error::Decode(m) => write!(f, "decode error: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<peios::Error> for Error {
    fn from(e: peios::Error) -> Self {
        // The new `peios::Error` is an `io::Error`-like wrapper over `errno`;
        // it no longer distinguishes the old `QueryTruncated` /
        // `UnknownDiscriminant` decode variants, so every peios error maps to a
        // syscall error carrying its raw errno.
        match e.raw_os_error() {
            Some(errno) => Error::Syscall {
                op: "peios syscall",
                errno,
                detail: None,
            },
            None => Error::Syscall {
                op: "peios",
                errno: 0,
                detail: Some(e.to_string()),
            },
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
