// spell-checker:ignore (libs) mkirf initramfs cpio
//! Error type for pu_mkirf.
//!
//! Mirrors the peiosutils applet convention (see `regman::error`): a
//! `thiserror` enum with a stable `exit_code`, surfaced to the multi-call
//! runtime as a `uucore` error in `uumain`. Operational failures share exit
//! 1; only a malformed invocation that clap cannot express is exit 2 (clap's
//! own usage-error code).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// A semantic invocation error clap cannot catch (e.g. `--watch` with the
    /// output inside the watched tree). Exit 2, matching clap's usage code.
    #[error("{0}")]
    Usage(String),

    /// The source tree is not a bootable initramfs layout — not a directory,
    /// no executable `/init`, or a stray `hooks.seq`. Exit 1.
    #[error("{0}")]
    Layout(String),

    /// A hook block is malformed or the hook DAG cannot be ordered (a cycle,
    /// an unknown reference). Exit 1.
    #[error("{0}")]
    Hooks(String),

    /// A filesystem or compression failure. Exit 1.
    #[error("{0}")]
    Io(String),
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::Layout(_) | Self::Hooks(_) | Self::Io(_) => 1,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
