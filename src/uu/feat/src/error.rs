// feat's error type and exit-code policy.
//
//   0  success
//   1  usage / argument problem        Error::Usage
//   2  no such feature                 Error::NotFound
//   3  KACS denied (EPERM/EACCES)      Error::Denied
//   4  feature in the wrong state      Error::State   (e.g. enable before install)
//   5  script failed / I/O / syscall   Error::Script, Error::Io, Error::Registry

use std::fmt;

const EPERM: i32 = 1;
const ENOENT: i32 = 2;
const EACCES: i32 = 13;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// Usage / argument problem (exit 1).
    Usage(String),
    /// No such feature directory (exit 2).
    NotFound(String),
    /// KACS denied a registry write or the script (exit 3).
    Denied { op: String, errno: i32 },
    /// Feature is in the wrong state for the requested transition (exit 4).
    State(String),
    /// A lifecycle script exited non-zero (exit 5).
    Script { feature: String, phase: &'static str, code: Option<i32> },
    /// Filesystem / process I/O failure (exit 5).
    Io { op: String, source: std::io::Error },
    /// Registry operation failed (exit 5, unless reclassified).
    Registry { op: String, errno: i32 },
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) => 1,
            Error::NotFound(_) => 2,
            Error::Denied { .. } => 3,
            Error::State(_) => 4,
            Error::Script { .. } | Error::Io { .. } | Error::Registry { .. } => 5,
        }
    }

    /// Funnel a `peios::Error` from registry op `op` into the right bucket so the
    /// errno -> exit-code policy lives in one place (mirrors reg's `from_peios`).
    pub fn from_peios(op: impl Into<String>, e: peios::Error) -> Error {
        let op = op.into();
        match e.raw_os_error().unwrap_or(0) {
            EACCES | EPERM => Error::Denied {
                op,
                errno: e.raw_os_error().unwrap_or(0),
            },
            errno => Error::Registry { op, errno },
        }
    }

    /// True when this peios error means "the key/value does not exist".
    pub fn is_enoent(e: &peios::Error) -> bool {
        e.raw_os_error() == Some(ENOENT)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(m) => write!(f, "{m}"),
            Error::NotFound(name) => write!(f, "no such feature: {name}"),
            Error::Denied { op, errno } => {
                write!(f, "{op}: denied (errno {errno}) — your token lacks the authority")
            }
            Error::State(m) => write!(f, "{m}"),
            Error::Script { feature, phase, code } => match code {
                Some(c) => write!(f, "feature {feature}: {phase} script failed (exit {c})"),
                None => write!(f, "feature {feature}: {phase} script killed by signal"),
            },
            Error::Io { op, source } => write!(f, "{op}: {source}"),
            Error::Registry { op, errno } => write!(f, "{op}: registry error (errno {errno})"),
        }
    }
}
