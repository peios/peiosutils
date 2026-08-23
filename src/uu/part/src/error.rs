// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Error type for `part`.
//!
//! The variants are shaped around one idea: a tool that can destroy a disk
//! should never report a *refusal* in the same words as a *failure*. Refusing
//! to touch an MBR disk is `part` working correctly; failing to read a header
//! is not. They exit with different codes so a script can tell them apart
//! without parsing prose.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, PartError>;

#[derive(Debug, Error)]
pub enum PartError {
    /// Bad invocation, or a request that cannot be satisfied as asked.
    #[error("{0}")]
    Usage(String),

    /// The device could not be read or written.
    #[error("{path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// There is no GPT here. Distinct from `Damaged`: this disk simply is not
    /// one, which for `create` is the ordinary case.
    #[error("no GPT partition table")]
    NoGpt,

    /// A GPT is present but structurally wrong. Reported, never silently
    /// repaired — `part` is not a recovery tool, and quietly rewriting a table
    /// nobody asked it to touch is how data is lost.
    #[error("damaged GPT: {0}")]
    Damaged(String),

    /// The disk carries something `part` will not manage. Held as data rather
    /// than a formatted string so the caller can name it precisely and suggest
    /// the right escape hatch.
    #[error("{0}")]
    Foreign(String),

    /// The requested size does not fit.
    #[error("no free extent holds {wanted} sectors (largest is {largest})")]
    NoSpace { wanted: u64, largest: u64 },

    /// A safety guard fired. Always a refusal, never a malfunction.
    #[error("{0}")]
    Refused(String),
}

impl PartError {
    /// Exit codes, chosen so callers can branch without reading the message:
    ///
    /// | code | meaning |
    /// |---|---|
    /// | 1 | usage, or the operation genuinely failed |
    /// | 2 | I/O against the device |
    /// | 3 | refused by a safety guard, including a foreign table |
    ///
    /// `peios-install` cares about 3 specifically: it means "the disk is not
    /// what you said it was", which is worth stopping an install for rather
    /// than retrying.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Io { .. } => 2,
            Self::Refused(_) | Self::Foreign(_) => 3,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_are_distinguishable_from_failures() {
        assert_eq!(PartError::Refused("mounted".into()).exit_code(), 3);
        assert_eq!(PartError::Foreign("dos".into()).exit_code(), 3);
        assert_eq!(PartError::Usage("bad".into()).exit_code(), 1);
        assert_eq!(PartError::NoGpt.exit_code(), 1);
        assert_eq!(
            PartError::Io {
                path: "/dev/null".into(),
                source: std::io::Error::other("x")
            }
            .exit_code(),
            2
        );
    }

    #[test]
    fn no_space_names_both_numbers() {
        let m = PartError::NoSpace {
            wanted: 100,
            largest: 40,
        }
        .to_string();
        assert!(m.contains("100") && m.contains("40"), "{m}");
    }
}
