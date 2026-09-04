// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Errors and the util-linux-compatible exit-code mapping (spec §10).
//!
//! | code | meaning                                   |
//! |------|-------------------------------------------|
//! | 1    | incorrect invocation **or permissions**   |
//! | 2    | system error (ENOMEM, no free loop, …)    |
//! | 4    | internal bug (invariant failure)          |
//! | 8    | user interrupt (SIGINT)                   |
//! | 32   | mount failure                             |
//! | 64   | some succeeded, some failed (multi-arg)   |
//! | 126  | helper exec failure                       |
//!
//! Code 16 (mtab) is never produced (no mtab on peios).

use std::io;

/// An OS error that also carries the kernel's explanation of it.
///
/// The new mount API reports *why* a step failed through the fs context's
/// log (`read(2)` on the `fsopen` fd), which is far more useful to a person
/// than the bare errno — "Can't open blockdev" beside `EROFS` says which
/// layer refused. But `io::Error::new(kind, text)` is a *custom* error, and
/// `raw_os_error()` on one is `None`. Wrapping that way silently detached
/// every annotated failure from the code that decides what to do with an
/// errno: `from_syscall` stopped classifying `EACCES`/`EPERM` as permission
/// failures, and the auto-ro fallback (§2.5.1) stopped recognising `EROFS`
/// as write-protect, so `mount -t iso9660 -o ro /dev/sr0` failed outright
/// on a CD-ROM even though the retry it needed was one branch away.
///
/// Keep the errno alongside the text, and let [`os_code`] see through it.
#[derive(Debug)]
pub struct AnnotatedOsError {
    pub code: i32,
    pub detail: String,
}

impl std::fmt::Display for AnnotatedOsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({})",
            self.detail,
            io::Error::from_raw_os_error(self.code)
        )
    }
}

impl std::error::Error for AnnotatedOsError {}

/// Attach the kernel's explanation to an OS error without losing its errno.
/// A `source` that is not an OS error (no errno to keep) is returned as is.
pub fn annotate(source: io::Error, detail: &str) -> io::Error {
    match source.raw_os_error() {
        Some(code) => io::Error::new(
            source.kind(),
            AnnotatedOsError {
                code,
                detail: detail.to_string(),
            },
        ),
        None => source,
    }
}

/// The errno behind an `io::Error`, whether it is a plain OS error or one
/// [`annotate`] wrapped. Every errno-driven decision in the tool goes
/// through here rather than `raw_os_error()`.
pub fn os_code(e: &io::Error) -> Option<i32> {
    e.raw_os_error().or_else(|| {
        e.get_ref()
            .and_then(|inner| inner.downcast_ref::<AnnotatedOsError>())
            .map(|a| a.code)
    })
}

/// A mount/umount operation failure.
#[derive(Debug, thiserror::Error)]
pub enum MountError {
    /// Bad invocation: incompatible flags, missing operands, unparsable `-o`,
    /// invalid `policy=`/`--synth-sddl`, single-operand-without-fstab,
    /// embedded-NUL path, "no helper for type". Exit 1.
    #[error("{0}")]
    Usage(String),

    /// Authorization denial (`EPERM`/`EACCES`) surfaced from a syscall. Exit 1
    /// (util-linux folds permission failures into code 1, not 32).
    #[error("{stage}: {source}")]
    Permission {
        stage: &'static str,
        #[source]
        source: io::Error,
    },

    /// A surface not yet implemented. Honest rather than silently wrong. Exit 1.
    #[error("{0}: not yet implemented")]
    Unimplemented(String),

    /// The mount/unmount itself failed — a syscall in the flow errored. `stage`
    /// names the syscall so failures are attributable to the right layer. Exit
    /// 32.
    #[error("{stage}: {source}")]
    Mount {
        stage: &'static str,
        #[source]
        source: io::Error,
    },

    /// A system-level failure unrelated to the mount itself (`ENOMEM`, no free
    /// loop device, fork failure). Exit 2.
    #[error("{0}")]
    System(String),

    /// An external `mount.<type>`/`umount.<type>` helper failed to exec. Exit
    /// 126. (Moot until a helper ships, §9.)
    #[error("{0}")]
    Helper(String),

    /// An internal invariant failure. Exit 4.
    #[error("internal error: {0}")]
    Bug(String),

    /// User interrupt (SIGINT), after signal-safe cleanup. Exit 8.
    #[error("interrupted")]
    Interrupted,

    /// Aggregate across several source/target arguments: at least one succeeded
    /// and at least one failed. Exit 64. (Multi-arg umount, §10.)
    #[error("{0}")]
    Partial(String),

    /// The umount target is not mounted / absent. Exit 32 (unless `-g`, which
    /// the caller handles before constructing this).
    #[error("{0}")]
    NotMounted(String),
}

impl MountError {
    /// The util-linux-compatible process exit code.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) | Self::Permission { .. } | Self::Unimplemented(_) => 1,
            Self::System(_) => 2,
            Self::Bug(_) => 4,
            Self::Interrupted => 8,
            Self::Mount { .. } | Self::NotMounted(_) => 32,
            Self::Partial(_) => 64,
            Self::Helper(_) => 126,
        }
    }

    /// Map an `io::Error` from a named syscall stage to the right code: an
    /// authorization errno (`EPERM`/`EACCES`) becomes [`Self::Permission`]
    /// (exit 1); anything else is a [`Self::Mount`] failure (exit 32). This is
    /// the §10 "permissions fold into code 1" rule applied at the syscall edge.
    pub fn from_syscall(stage: &'static str, source: io::Error) -> Self {
        match os_code(&source) {
            Some(libc::EPERM) | Some(libc::EACCES) => Self::Permission { stage, source },
            _ => Self::Mount { stage, source },
        }
    }

    /// Curried form for `.map_err(MountError::at("fsopen"))`.
    pub fn at(stage: &'static str) -> impl FnOnce(io::Error) -> Self {
        move |source| Self::from_syscall(stage, source)
    }
}

/// Result alias for the mount library.
pub type Result<T> = std::result::Result<T, MountError>;
