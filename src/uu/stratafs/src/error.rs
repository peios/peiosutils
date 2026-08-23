use std::fmt;
use std::path::Path;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io {
        operation: String,
        source: std::io::Error,
    },
    InvalidMountTable(String),
    NoMounts,
    NotStratafs(String),
    NotVisible {
        path: String,
        source: std::io::Error,
    },
    Unsupported(String),
}

impl Error {
    pub fn io(operation: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            operation: operation.into(),
            source,
        }
    }

    pub fn path_io(operation: &str, path: &Path, source: std::io::Error) -> Self {
        Self::io(
            format!("{operation} {}", super::model::display_path(path)),
            source,
        )
    }

    pub const fn exit_code(&self) -> i32 {
        2
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
            Self::InvalidMountTable(message) => {
                write!(f, "invalid StrataFS mount table entry: {message}")
            }
            Self::NoMounts => write!(f, "no StrataFS mounts found"),
            Self::NotStratafs(path) => write!(f, "not on a StrataFS mount: {path}"),
            Self::NotVisible { path, source } => {
                write!(f, "cannot see {path}: {source}; refusing a partial result")
            }
            Self::Unsupported(message) => f.write_str(message),
        }
    }
}
