// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Error type for `lsblk`.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, LsblkError>;

#[derive(Debug, Error)]
pub enum LsblkError {
    /// Bad invocation (unknown column, malformed major-number list, …). Maps to
    /// the util-linux lsblk "incorrect usage" exit code.
    #[error("{0}")]
    Usage(String),

    /// A system-level failure that aborts the whole run (e.g. /sys/block is
    /// unreadable). Per-device read failures are NOT this — they degrade the
    /// affected columns to empty/`?`, they do not abort.
    #[error("{0}")]
    System(String),
}

impl LsblkError {
    /// util-linux lsblk exits 1 on usage errors and on a failed run; we mirror
    /// that single non-zero code rather than inventing a richer scheme.
    pub fn exit_code(&self) -> i32 {
        1
    }
}
