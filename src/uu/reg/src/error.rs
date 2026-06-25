// reg's error type. Maps to the exit-code table in docs/reg-spec.md §9.
//
//   0  success            (not an error)
//   1  usage              Error::Usage
//   2  not found          Error::NotFound      (ENOENT)
//   3  access denied      Error::Denied        (EACCES, EPERM)
//   4  invalid spec       Error::InvalidSpec   (EINVAL, EXDEV, ENAMETOOLONG, …)
//   5  syscall / source   Error::Syscall       (EIO, ETIMEDOUT, ENOSPC, …)
//   6  CAS conflict       Error::Conflict      (EAGAIN, from --expected-seq)

use std::fmt;

const EPERM: i32 = 1;
const ENOENT: i32 = 2;
const EIO: i32 = 5;
const EAGAIN: i32 = 11;
const EACCES: i32 = 13;
const EEXIST: i32 = 17;
const EXDEV: i32 = 18;
const ENOTEMPTY: i32 = 39;
const EINVAL: i32 = 22;
const ENAMETOOLONG: i32 = 36;
const ELOOP: i32 = 40;

#[derive(Debug)]
pub enum Error {
    /// Usage / argument problem (exit 1).
    Usage(String),
    /// No such key or value (exit 2).
    NotFound(String),
    /// KACS denied the operation (exit 3).
    Denied {
        op: &'static str,
        target: String,
        errno: i32,
    },
    /// Invalid spec: bad path, type, literal, cross-hive txn, … (exit 4).
    InvalidSpec(String),
    /// Syscall / source failure (exit 5).
    Syscall {
        op: &'static str,
        errno: i32,
        detail: Option<String>,
    },
    /// Compare-and-swap conflict from `--expected-seq` (exit 6).
    Conflict { target: String },
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) => 1,
            Error::NotFound(_) => 2,
            Error::Denied { .. } => 3,
            Error::InvalidSpec(_) => 4,
            Error::Syscall { .. } => 5,
            Error::Conflict { .. } => 6,
        }
    }

    /// Classify a `peios::Error` from operation `op` against `target` into the
    /// right exit-code bucket. This is the single funnel every registry call
    /// goes through, so the errno→exit-code policy lives in one place.
    pub fn from_peios(op: &'static str, target: &str, e: peios::Error) -> Error {
        let errno = e.raw_os_error().unwrap_or(0);
        match errno {
            ENOENT => Error::NotFound(target.to_string()),
            EACCES | EPERM => Error::Denied {
                op,
                target: target.to_string(),
                errno,
            },
            EAGAIN => Error::Conflict {
                target: target.to_string(),
            },
            EINVAL | EXDEV | ENAMETOOLONG | ELOOP | EEXIST | ENOTEMPTY => Error::InvalidSpec(
                format!("{op}: {} ({target})", peios::Error::from_raw_os_error(errno)),
            ),
            _ => Error::Syscall {
                op,
                errno: if errno == 0 { EIO } else { errno },
                detail: Some(target.to_string()),
            },
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(m) => write!(f, "{m}"),
            Error::NotFound(m) => write!(f, "not found: {m}"),
            Error::Denied { op, target, errno } => {
                let e = peios::Error::from_raw_os_error(*errno);
                write!(f, "{op}: {e}\n  target: {target}")
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
            Error::Conflict { target } => write!(
                f,
                "value changed under you (sequence mismatch): {target}\n  \
                 re-read and retry, or drop --expected-seq to force"
            ),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
