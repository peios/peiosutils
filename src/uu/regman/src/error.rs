// Error type for pu_regman.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Bad flags, conflicting options, malformed invocation. Exit 1.
    #[error("usage: {0}")]
    Usage(String),

    /// No documentation found for the requested path/value. Exit 2.
    #[error("no manual entry for {0}")]
    NotFound(String),

    /// A fragment was malformed, or `lint` found problems. Exit 3.
    #[error("{0}")]
    Fragment(String),

    /// Filesystem / IO failure. Exit 4.
    #[error("{0}")]
    Io(String),
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 1,
            Self::NotFound(_) => 2,
            Self::Fragment(_) => 3,
            Self::Io(_) => 4,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
