// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Attribute preservation for Peios file operations (`--preserve` & friends).
//!
//! When a file operation creates a *new* inode — `cp`, or a cross-filesystem
//! `mv` — the new object gets a fresh security descriptor inherited from its
//! destination directory. `--preserve` lets the caller instead carry chosen
//! attributes (security-descriptor components, timestamps, xattrs, hardlink
//! structure) from the source.
//!
//! This module is shared verbatim by `pu_cp` and `pu_mv` so the two commands
//! expose an identical `--preserve` surface. It owns three things:
//!
//!  - [`Attributes`] / [`Preserve`] — the preservation model;
//!  - [`resolve`] — turning parsed clap matches into an [`Attributes`];
//!  - [`copy_attributes`] — applying an [`Attributes`] from source to dest.
//!
//! Most attributes a caller requests are `Preserve::Yes { required: true }`:
//! a requested preserve that cannot be honoured (e.g. carrying a SACL without
//! `SeSecurityPrivilege`) is a hard error, not a warning. The exception is the
//! implicit `exec` axis (see [`Attributes::IMPLICIT`]): a plain `cp` preserves
//! executable-ness best-effort (`required: false`) so it never fails the copy,
//! while any explicit request for it (`-p`, `-a`, `--preserve=exec`) is
//! `required: true`. The `required: false` path also backs a future
//! `--soft-preserve`.

use std::cmp::Ordering;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, Metadata, OpenOptions, Permissions};
use std::io;
use std::os::fd::AsFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

use clap::ArgMatches;
use filetime::FileTime;
use peios::file::{self, OpenFlags, SecInfo};
use peios::security::{Control, SdView, SecurityDescriptor, strip_inherited};

use crate::display::Quotable;
use crate::error::UError;

/// Error type for preservation operations.
///
/// Deliberately small: callers (`cp`, `mv`) convert it into their own error
/// type. The `io::Error` is kept inline (rather than stringified) so
/// [`is_enotsup_error`] can still recognise unsupported-operation failures.
#[derive(Debug)]
pub enum PreserveError {
    /// Bare I/O error.
    Io(io::Error),
    /// I/O error with a `context: error` prefix.
    IoContext(io::Error, String),
    /// Any other preservation failure (SD copy, invalid attribute name, ...).
    Other(String),
}

impl fmt::Display for PreserveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::IoContext(e, ctx) => write!(f, "{ctx}: {}", crate::error::strip_errno(e)),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for PreserveError {}

impl UError for PreserveError {
    fn code(&self) -> i32 {
        1
    }
}

impl From<io::Error> for PreserveError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Result type for preservation operations.
pub type PreserveResult<T> = Result<T, PreserveError>;

/// Whether a single attribute should be preserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preserve {
    /// Not preserved. `explicit` records whether `--no-preserve` named it
    /// (vs. it simply defaulting off) — needed to distinguish the two.
    No { explicit: bool },
    /// Preserved. `required` decides whether a failure to preserve is fatal
    /// (`true`) or merely a warning (`false`).
    Yes { required: bool },
}

impl PartialOrd for Preserve {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Preserve {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::No { .. }, Self::No { .. }) => Ordering::Equal,
            (Self::Yes { .. }, Self::No { .. }) => Ordering::Greater,
            (Self::No { .. }, Self::Yes { .. }) => Ordering::Less,
            (
                Self::Yes { required: req_self },
                Self::Yes {
                    required: req_other,
                },
            ) => req_self.cmp(req_other),
        }
    }
}

/// Preservation settings: one [`Preserve`] per attribute.
///
/// Derived from options as follows:
///
///  - `--preserve=ATTR_LIST` → parse with [`Attributes::parse_iter`]
///  - `-p` → [`Attributes::DEFAULT`] (timestamps only — xcopy-style)
///  - `-a`/`--archive` or `--preserve-all` → [`Attributes::ALL`]
///  - `--sd` → [`Attributes::SD`]
///  - `--sd-explicit` → [`Attributes::SD_EXPLICIT`]
///  - `-d` → [`Attributes::LINKS`]
///  - otherwise → [`Attributes::NONE`]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Attributes {
    /// Owner SID component of the security descriptor.
    pub owner: Preserve,
    /// Full DACL (explicit + inherited ACEs).
    pub dacl: Preserve,
    /// Full SACL (explicit + inherited ACEs; includes mandatory labels per NT model).
    pub sacl: Preserve,
    /// DACL with inherited ACEs stripped.
    pub daclni: Preserve,
    /// SACL with inherited ACEs stripped.
    pub saclni: Preserve,
    /// atime / mtime.
    pub timestamps: Preserve,
    /// Hardlink structure across multi-source/recursive copies.
    pub links: Preserve,
    /// `security.peios.*` namespace excluding `security.peios.sd`
    /// (which is preserved via owner/dacl/sacl). Includes things like
    /// `security.peios.fevm` and Linux-compat security xattrs (SELinux
    /// contexts, IMA/EVM signatures).
    pub security: Preserve,
    /// All other extended attributes (`user.*`, `trusted.*`, `system.*`).
    pub xattrs: Preserve,
    /// Executable-ness: whether the source has any POSIX execute bit. Peios
    /// has no meaningful read/write mode bits (CAP_DAC_OVERRIDE waives them;
    /// KACS is the access gate), but the kernel's DAC layer still vetoes
    /// `execve` on a regular file with zero execute bits even for root — so
    /// this one bit must travel. Any source exec bit → all three set on the
    /// dest; none → cleared. Not a per-class permission; an intrinsic
    /// "is this a program".
    pub exec: Preserve,
}

impl Default for Attributes {
    fn default() -> Self {
        Self::NONE
    }
}

/// Attribute names accepted in `--preserve=`/`--no-preserve=` lists.
pub static PRESERVABLE_ATTRIBUTES: &[&str] = &[
    "owner",
    "dacl",
    "sacl",
    "daclni",
    "saclni",
    "timestamps",
    "links",
    "exec",
    "security",
    "xattrs",
    "xattr",
];

/// Default attributes for bare `--preserve` (no value).
pub const PRESERVE_DEFAULT_VALUES: &str = "timestamps";

impl Attributes {
    /// Preserve nothing.
    pub const NONE: Self = Self {
        owner: Preserve::No { explicit: false },
        dacl: Preserve::No { explicit: false },
        sacl: Preserve::No { explicit: false },
        daclni: Preserve::No { explicit: false },
        saclni: Preserve::No { explicit: false },
        timestamps: Preserve::No { explicit: false },
        links: Preserve::No { explicit: false },
        security: Preserve::No { explicit: false },
        xattrs: Preserve::No { explicit: false },
        exec: Preserve::No { explicit: false },
    };

    /// Every preservable attribute, with `required: true`.
    /// Reachable via `--preserve-all` or `-a`/`--archive`.
    pub const ALL: Self = Self {
        owner: Preserve::Yes { required: true },
        dacl: Preserve::Yes { required: true },
        sacl: Preserve::Yes { required: true },
        daclni: Preserve::Yes { required: true },
        saclni: Preserve::Yes { required: true },
        timestamps: Preserve::Yes { required: true },
        links: Preserve::Yes { required: true },
        security: Preserve::Yes { required: true },
        xattrs: Preserve::Yes { required: true },
        exec: Preserve::Yes { required: true },
    };

    /// Default for `-p`: timestamps + exec (both required). exec rides along
    /// because executable-ness is an intrinsic file property, not a permission.
    pub const DEFAULT: Self = Self {
        timestamps: Preserve::Yes { required: true },
        exec: Preserve::Yes { required: true },
        ..Self::NONE
    };

    /// The implicit baseline when no explicit `--preserve=` list is given:
    /// preserve exec best-effort (`required: false`) and nothing else. A plain
    /// `cp` keeps programs executable but never fails on it; an explicit
    /// `--preserve=LIST` starts from [`NONE`] instead, so exec is then on only
    /// if the list names it.
    pub const IMPLICIT: Self = Self {
        exec: Preserve::Yes { required: false },
        ..Self::NONE
    };

    /// `--sd`: full security descriptor (owner + dacl + sacl).
    pub const SD: Self = Self {
        owner: Preserve::Yes { required: true },
        dacl: Preserve::Yes { required: true },
        sacl: Preserve::Yes { required: true },
        ..Self::NONE
    };

    /// `--sd-explicit`: SD with no-inherited DACL/SACL variants (carries the
    /// source's *explicit* ACEs and lets the destination's parent supply its
    /// own inheritance).
    pub const SD_EXPLICIT: Self = Self {
        owner: Preserve::Yes { required: true },
        daclni: Preserve::Yes { required: true },
        saclni: Preserve::Yes { required: true },
        ..Self::NONE
    };

    /// `-d`: hardlink structure only.
    pub const LINKS: Self = Self {
        links: Preserve::Yes { required: true },
        ..Self::NONE
    };

    /// Field-wise maximum: the stronger [`Preserve`] wins for each attribute.
    #[must_use]
    pub fn union(self, other: &Self) -> Self {
        Self {
            owner: self.owner.max(other.owner),
            dacl: self.dacl.max(other.dacl),
            sacl: self.sacl.max(other.sacl),
            daclni: self.daclni.max(other.daclni),
            saclni: self.saclni.max(other.saclni),
            timestamps: self.timestamps.max(other.timestamps),
            links: self.links.max(other.links),
            security: self.security.max(other.security),
            xattrs: self.xattrs.max(other.xattrs),
            exec: self.exec.max(other.exec),
        }
    }

    /// Set fields to `Preserve::No { explicit: true }` where `other` requests
    /// them. Used by `--no-preserve=...`.
    #[must_use]
    pub fn diff(self, other: &Self) -> Self {
        fn update_preserve_field(current: Preserve, other: Preserve) -> Preserve {
            if matches!(other, Preserve::Yes { .. }) {
                Preserve::No { explicit: true }
            } else {
                current
            }
        }
        Self {
            owner: update_preserve_field(self.owner, other.owner),
            dacl: update_preserve_field(self.dacl, other.dacl),
            sacl: update_preserve_field(self.sacl, other.sacl),
            daclni: update_preserve_field(self.daclni, other.daclni),
            saclni: update_preserve_field(self.saclni, other.saclni),
            timestamps: update_preserve_field(self.timestamps, other.timestamps),
            links: update_preserve_field(self.links, other.links),
            security: update_preserve_field(self.security, other.security),
            xattrs: update_preserve_field(self.xattrs, other.xattrs),
            exec: update_preserve_field(self.exec, other.exec),
        }
    }

    /// Parse an iterator of attribute names into an [`Attributes`].
    pub fn parse_iter<T>(values: impl Iterator<Item = T>) -> PreserveResult<Self>
    where
        T: AsRef<str>,
    {
        let mut new = Self::NONE;
        for value in values {
            new = new.union(&Self::parse_single_string(value.as_ref())?);
        }
        Ok(new)
    }

    fn parse_single_string(value: &str) -> PreserveResult<Self> {
        let value = value.to_lowercase();

        let mut new = Self::NONE;
        let attribute = match value.as_ref() {
            "owner" => &mut new.owner,
            "dacl" => &mut new.dacl,
            "sacl" => &mut new.sacl,
            "daclni" => &mut new.daclni,
            "saclni" => &mut new.saclni,
            "timestamps" => &mut new.timestamps,
            "link" | "links" => &mut new.links,
            "exec" => &mut new.exec,
            "security" => &mut new.security,
            "xattrs" | "xattr" => &mut new.xattrs,
            _ => {
                return Err(PreserveError::Other(format!(
                    "invalid attribute {}",
                    value.quote()
                )));
            }
        };

        *attribute = Preserve::Yes { required: true };

        Ok(new)
    }
}

/// One preserve-related command-line option, classified by what it does to
/// the running [`Attributes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreserveOpt {
    /// `--preserve[=LIST]` — value-bearing; unions in the parsed list.
    Preserve,
    /// `--no-preserve=LIST` — value-bearing; diffs out the parsed list.
    NoPreserve,
    /// `-a`/`--archive` or `--preserve-all` — resets to [`Attributes::ALL`].
    All,
    /// `-p` — unions in [`Attributes::DEFAULT`].
    Default,
    /// `--sd` — unions in [`Attributes::SD`].
    Sd,
    /// `--sd-explicit` — unions in [`Attributes::SD_EXPLICIT`].
    SdExplicit,
    /// `-d` — unions in [`Attributes::LINKS`].
    Links,
}

/// Resolve preserve-related options into an [`Attributes`].
///
/// `options` maps each clap argument id to its [`PreserveOpt`] kind. Options
/// are applied in the order they appeared on the command line, so a later
/// flag overrides an earlier one (POSIX `cp` semantics); `-a` expanding to
/// `-dR --preserve=all` and repeated flags are both handled by sorting on the
/// clap value index.
pub fn resolve(
    matches: &ArgMatches,
    options: &[(&str, PreserveOpt)],
) -> PreserveResult<Attributes> {
    // (command-line index, kind, values) for each occurrence of each option.
    let mut overriding_order: Vec<(usize, PreserveOpt, Vec<&String>)> = vec![];

    for &(name, opt) in options {
        match opt {
            PreserveOpt::Preserve | PreserveOpt::NoPreserve => {
                // Value-bearing, `ArgAction::Append`: walk each occurrence
                // with its values. `indices_of` yields per-value indices, so
                // after taking an occurrence's first index we skip the rest.
                if let (Some(occurrences), Some(mut indices)) = (
                    matches.get_occurrences::<String>(name),
                    matches.indices_of(name),
                ) {
                    occurrences.for_each(|val| {
                        if let Some(index) = indices.next() {
                            let val = val.collect::<Vec<&String>>();
                            for _ in 1..val.len() {
                                indices.next();
                            }
                            overriding_order.push((index, opt, val));
                        }
                    });
                }
            }
            _ => {
                // Boolean flag: a single index, no values.
                if let (Ok(Some(&true)), Some(index)) =
                    (matches.try_get_one::<bool>(name), matches.index_of(name))
                {
                    overriding_order.push((index, opt, vec![]));
                }
            }
        }
    }
    overriding_order.sort_by_key(|a| a.0);

    // exec is preserved best-effort by default (the IMPLICIT baseline). An
    // explicit `--preserve=LIST` is the user declaring the exact set, so it
    // resets the baseline to NONE — exec then survives only if listed.
    let explicit_preserve = overriding_order
        .iter()
        .any(|(_, opt, _)| matches!(opt, PreserveOpt::Preserve));
    let mut attributes = if explicit_preserve {
        Attributes::NONE
    } else {
        Attributes::IMPLICIT
    };
    for (_, opt, val) in overriding_order {
        match opt {
            PreserveOpt::All => attributes = Attributes::ALL,
            PreserveOpt::Sd => attributes = attributes.union(&Attributes::SD),
            PreserveOpt::SdExplicit => attributes = attributes.union(&Attributes::SD_EXPLICIT),
            PreserveOpt::Default => attributes = attributes.union(&Attributes::DEFAULT),
            PreserveOpt::Links => attributes = attributes.union(&Attributes::LINKS),
            PreserveOpt::Preserve => {
                attributes = attributes.union(&Attributes::parse_iter(val.into_iter())?);
            }
            PreserveOpt::NoPreserve if !val.is_empty() => {
                attributes = attributes.diff(&Attributes::parse_iter(val.into_iter())?);
            }
            PreserveOpt::NoPreserve => {}
        }
    }
    Ok(attributes)
}

/// Check if an error is ENOTSUP/EOPNOTSUPP (operation not supported).
/// Used to suppress xattr errors on filesystems that don't support them.
fn is_enotsup_error(error: &PreserveError) -> bool {
    #[cfg(unix)]
    const EOPNOTSUPP: i32 = libc::EOPNOTSUPP;
    #[cfg(not(unix))]
    const EOPNOTSUPP: i32 = 95;

    match error {
        PreserveError::Io(e) | PreserveError::IoContext(e, _) => {
            e.raw_os_error() == Some(EOPNOTSUPP)
        }
        PreserveError::Other(_) => false,
    }
}

/// Report a non-fatal preservation error to the user.
fn show_preserve_error(error: &PreserveError) {
    crate::show_error!("{error}");
}

/// Run a preservation step `f` for attribute `p`.
///
/// If `p` is `Yes { required: true }` a failure propagates. If it is
/// `Yes { required: false }` a failure is reported (unless it is merely an
/// unsupported-operation error) and swallowed. `No` does nothing.
fn handle_preserve<F: Fn() -> PreserveResult<()>>(p: Preserve, f: F) -> PreserveResult<()> {
    match p {
        Preserve::No { .. } => {}
        Preserve::Yes { required } => {
            let result = f();
            if required {
                result?;
            } else if let Err(ref error) = result {
                if !is_enotsup_error(error) {
                    show_preserve_error(error);
                }
            }
        }
    }
    Ok(())
}

/// `AT_SYMLINK_NOFOLLOW`, for the `at_flags` argument of the path forms of
/// [`peios::file::get_sd`] / [`peios::file::set_sd`].
fn at_nofollow() -> i32 {
    OpenFlags::SYMLINK_NOFOLLOW.bits() as i32
}

/// One end of an attribute copy, pinned to a single inode.
///
/// [`copy_attributes`] used to re-resolve `source` and `dest` for every step —
/// an `lstat`, a `chmod`, a `listxattr`, a `getxattr`, a `setxattr` and a
/// `kacs_get_sd`/`kacs_set_sd` pair, each its own path→inode lookup, all of
/// them made *after* the data copy had already finished. An attacker who can
/// write the destination directory can swap the destination for a symlink in
/// any one of those gaps and redirect the write. The `chmod` was the sharp
/// end, because `chmod(2)` follows symlinks: a privileged `cp` running in an
/// attacker-writable directory could be aimed at an arbitrary file
/// (GHSA-8r5f-98ww-c4c5, GHSA-p9fh-vm43-9xxc).
///
/// A `FileHandle` resolves the name once, with `O_NOFOLLOW`, and drives every
/// later step off that descriptor: `fstat`, `fchmod`, `f{list,get,set}xattr`
/// and `peios::file::fd_{get,set}_sd`.
///
/// Only regular files and directories are opened. A symlink cannot be opened
/// for I/O; a socket cannot be opened at all (`ENXIO`); opening a FIFO blocks
/// or has side effects, and opening a device node is visible to its driver.
/// Those keep the path forms — but only the ones that cannot be redirected
/// through a symlink swapped in at that name: `AT_SYMLINK_NOFOLLOW` for the
/// security descriptor, `l*xattr` for extended attributes (which is what the
/// `xattr` crate's non-`_deref` functions are), `utimensat(AT_SYMLINK_NOFOLLOW)`
/// for timestamps, and no `chmod` at all.
struct FileHandle<'a> {
    /// The name this handle came from. Used for diagnostics, and for the
    /// no-follow path fallbacks when there is no descriptor.
    path: &'a Path,
    /// The pinned descriptor, for a regular file or a directory.
    file: Option<File>,
    /// `fstat` of `file`, or the `lstat` of `path` when there is no `file`.
    metadata: Metadata,
}

impl<'a> FileHandle<'a> {
    /// Pin `path`. Errors are labelled with the caller's `source -> dest`
    /// `context`.
    fn open(path: &'a Path, context: &str) -> PreserveResult<Self> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|e| PreserveError::IoContext(e, context.to_string()))?;
        let file_type = metadata.file_type();
        if !(file_type.is_file() || file_type.is_dir()) {
            return Ok(Self {
                path,
                file: None,
                metadata,
            });
        }
        match Self::open_nofollow(path, file_type.is_dir()) {
            Ok(file) => {
                let metadata = file
                    .metadata()
                    .map_err(|e| PreserveError::IoContext(e, context.to_string()))?;
                Ok(Self {
                    path,
                    file: Some(file),
                    metadata,
                })
            }
            // `ELOOP` from an `O_NOFOLLOW` open of something the `lstat` above
            // saw as a regular file or a directory means the name was swapped
            // for a symlink in between. Fail closed: falling back to path
            // operations here would follow exactly the link we just caught.
            Err(e) if e.raw_os_error() == Some(libc::ELOOP) => Err(PreserveError::Other(format!(
                "{}: replaced by a symbolic link while copying; refusing to preserve attributes through it",
                path.quote()
            ))),
            // Nameable but not openable. Fall back to the no-follow path
            // forms: still unredirectable, but the `chmod` that widens a
            // read-only destination for `setxattr` is skipped, so preserving
            // xattrs onto such an object can fail where it once succeeded.
            Err(_) => Ok(Self {
                path,
                file: None,
                metadata,
            }),
        }
    }

    fn open_nofollow(path: &Path, is_dir: bool) -> io::Result<File> {
        let mut custom = libc::O_NOFOLLOW | libc::O_CLOEXEC;
        if is_dir {
            custom |= libc::O_DIRECTORY;
        }
        let open = |write: bool| {
            OpenOptions::new()
                .read(!write)
                .write(write)
                .custom_flags(custom)
                .open(path)
        };
        match open(false) {
            // A destination with no read bit for us is still ours to `fchmod`
            // and to stamp; a write-only open pins the same inode just as
            // well. `O_WRONLY` is invalid on a directory, hence the gate.
            Err(e) if !is_dir && e.raw_os_error() == Some(libc::EACCES) => open(true),
            res => res,
        }
    }

    /// The metadata captured when the handle was opened.
    fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    fn is_symlink(&self) -> bool {
        self.metadata.file_type().is_symlink()
    }

    /// The pinned object's mode *now* — the security-descriptor copy can
    /// synthesise mode bits, so the callers that care re-read it rather than
    /// trusting the mode captured at open time.
    fn current_mode(&self) -> io::Result<u32> {
        match &self.file {
            Some(file) => Ok(file.metadata()?.permissions().mode()),
            None => Ok(self.metadata.permissions().mode()),
        }
    }

    /// `fchmod` through the pinned descriptor.
    ///
    /// Deliberately a no-op when there is none: a `chmod` by path follows
    /// symlinks, and that redirect is the whole of GHSA-8r5f-98ww-c4c5.
    /// Everything without a descriptor is a symlink, a socket, a FIFO or a
    /// device node, and none of those carries mode bits `--preserve` is
    /// about (`exec` is a property of programs, and a symlink's mode is not
    /// settable on Linux at all).
    fn set_mode(&self, mode: u32) -> io::Result<()> {
        match &self.file {
            Some(file) => file.set_permissions(Permissions::from_mode(mode)),
            None => Ok(()),
        }
    }

    #[cfg(all(unix, not(target_os = "android")))]
    fn list_xattrs(&self) -> io::Result<Vec<OsString>> {
        use xattr::FileExt;
        match &self.file {
            Some(file) => Ok(file.list_xattr()?.collect()),
            None => Ok(xattr::list(self.path)?.collect()),
        }
    }

    #[cfg(all(unix, not(target_os = "android")))]
    fn get_xattr(&self, name: &OsString) -> io::Result<Option<Vec<u8>>> {
        use xattr::FileExt;
        match &self.file {
            Some(file) => file.get_xattr(name),
            None => xattr::get(self.path, name),
        }
    }

    #[cfg(all(unix, not(target_os = "android")))]
    fn set_xattr(&self, name: &OsString, value: &[u8]) -> io::Result<()> {
        use xattr::FileExt;
        match &self.file {
            Some(file) => file.set_xattr(name, value),
            None => xattr::set(self.path, name, value),
        }
    }

    /// Read the `info` components of the pinned object's security descriptor.
    fn get_sd(&self, info: SecInfo) -> peios::Result<SecurityDescriptor> {
        match &self.file {
            Some(file) => file::fd_get_sd(file.as_fd(), info),
            None => file::get_sd(None, self.path, info, at_nofollow()),
        }
    }

    /// Write the `info` components of `sd` onto the pinned object.
    ///
    /// A symlink carries a descriptor of its own, and it is that descriptor
    /// the caller is copying: following the link would read and write the
    /// TARGET's instead, which is wrong twice over. It silently rewrites an
    /// object nobody asked about, and during a tree copy the target usually
    /// does not exist yet — `/init -> usr/bin/peinit2` is created long before
    /// `/usr` is — so the write fails outright with `ENOENT` and takes the
    /// whole copy down. Hence `AT_SYMLINK_NOFOLLOW` on the path form; it is
    /// inert on anything that is not a symlink, and the fd form cannot follow
    /// anything by construction.
    fn set_sd(&self, info: SecInfo, sd: &SecurityDescriptor) -> peios::Result<()> {
        match &self.file {
            Some(file) => file::fd_set_sd(file.as_fd(), info, sd),
            None => file::set_sd(None, self.path, info, sd, at_nofollow()),
        }
    }

    fn set_times(&self, atime: FileTime, mtime: FileTime) -> io::Result<()> {
        match &self.file {
            Some(file) => filetime::set_file_handle_times(file, Some(atime), Some(mtime)),
            None => filetime::set_symlink_file_times(self.path, atime, mtime),
        }
    }
}

/// Copy extended attributes (`user.*`, `trusted.*`, `system.*` — everything
/// outside the `security.` namespace) from `source` to `dest`.
#[cfg(all(unix, not(target_os = "android")))]
fn copy_extended_attrs(source: &FileHandle<'_>, dest: &FileHandle<'_>) -> PreserveResult<()> {
    // Security xattrs ride under `--preserve=security` (or under
    // owner/dacl/sacl for the SD itself), not here.
    copy_xattrs_filtered(source, dest, |name| {
        !name.as_encoded_bytes().starts_with(b"security.")
    })
}

/// Copy `security.*` xattrs from `source` to `dest`, excluding
/// `security.peios.sd` (which is preserved via the SD copy path).
#[cfg(all(unix, not(target_os = "android")))]
fn copy_security_xattrs(source: &FileHandle<'_>, dest: &FileHandle<'_>) -> PreserveResult<()> {
    copy_xattrs_filtered(source, dest, |name| {
        let bytes = name.as_encoded_bytes();
        bytes.starts_with(b"security.") && bytes != b"security.peios.sd"
    })
}

/// Walk `source`'s xattrs, copy those matching `keep` to `dest`. Temporarily
/// clears the readonly flag on `dest` if needed and restores it afterwards.
///
/// Every step runs through the pinned descriptors, so the destination cannot
/// be swapped underneath the widen/copy/restore sequence.
#[cfg(all(unix, not(target_os = "android")))]
fn copy_xattrs_filtered(
    source: &FileHandle<'_>,
    dest: &FileHandle<'_>,
    keep: impl Fn(&OsString) -> bool,
) -> PreserveResult<()> {
    // `Permissions::set_readonly(false)` is `mode |= 0o222` and
    // `set_readonly(true)` is `mode &= !0o222`; spelled out here because the
    // widen and the restore both go through `fchmod` now.
    let mode = dest.current_mode()?;
    let was_readonly = mode & 0o222 == 0;

    if was_readonly {
        dest.set_mode(mode | 0o222)?;
    }

    let result: PreserveResult<()> = (|| {
        // A source whose filesystem has no extended attributes at all has
        // nothing to preserve, so there is nothing here to fail. This is a
        // routine condition, not an edge case: squashfs images built without an
        // xattr table report ENOTSUP for every inode whose type carries a
        // `listxattr` op, FAT and 9p have no xattr channel, and overlayfs
        // forwards the lower layer's answer verbatim. Treating it as an error
        // made `cp -a` abort partway through any tree rooted on such a
        // filesystem. (GNU coreutils ignores ENOTSUP/ENOSYS here for the same
        // reason.)
        //
        // Scoped deliberately to the *listing*. ENOTSUP from `set` means the
        // source did have attributes and the destination cannot hold them —
        // that is a real failure to preserve, and stays one.
        let names = match source.list_xattrs() {
            Ok(names) => names,
            Err(e) if is_unsupported(&e) => return Ok(()),
            Err(e) => return Err(source_error(e, source.path)),
        };
        for attr_name in names {
            if !keep(&attr_name) {
                continue;
            }
            let value = source
                .get_xattr(&attr_name)
                .map_err(|e| source_error(e, source.path))?;
            if let Some(value) = value {
                dest.set_xattr(&attr_name, &value).map_err(|e| {
                    PreserveError::IoContext(
                        e,
                        format!("failed to set extended attributes on {}", dest.path.quote()),
                    )
                })?;
            }
        }
        Ok(())
    })();

    if was_readonly {
        dest.set_mode(dest.current_mode()? & !0o222)?;
    }

    result
}

/// Context for an xattr failure on the *source* side.
///
/// Worth its own helper because the whole read/write loop used to share the
/// destination's context string, so a failure to read the source was reported
/// against a destination path that was never touched.
#[cfg(all(unix, not(target_os = "android")))]
fn source_error(e: io::Error, source: &Path) -> PreserveError {
    PreserveError::IoContext(
        e,
        format!("failed to read extended attributes from {}", source.quote()),
    )
}

/// Whether an I/O error means "this filesystem does not do extended
/// attributes" rather than "this operation failed".
#[cfg(all(unix, not(target_os = "android")))]
fn is_unsupported(e: &io::Error) -> bool {
    matches!(
        e.raw_os_error(),
        Some(libc::EOPNOTSUPP) | Some(libc::ENOSYS)
    )
}

/// Mirror the source's executable-ness onto `dest`: if the source has any
/// POSIX execute bit set, set all three on `dest`; otherwise clear them.
/// Read/write bits are left untouched (irrelevant under CAP_DAC_OVERRIDE).
/// Symlinks are skipped — a symlink's exec-ness is its target's, and most
/// platforms can't chmod the link itself.
///
/// The `chmod` goes through `dest`'s pinned descriptor. The path form used to
/// live here, `lstat` then `set_permissions`, and it followed symlinks: a
/// destination swapped for a symlink between the two took the destination's
/// whole mode onto the link's target. This ran on *every* plain `cp`, since
/// [`Attributes::IMPLICIT`] preserves exec best-effort.
#[cfg(unix)]
fn apply_exec(source_metadata: &Metadata, dest: &FileHandle<'_>) -> PreserveResult<()> {
    if dest.is_symlink() {
        return Ok(());
    }
    let any_exec = source_metadata.permissions().mode() & 0o111 != 0;
    let mode = dest.current_mode()?;
    dest.set_mode((mode & !0o111) | if any_exec { 0o111 } else { 0 })?;
    Ok(())
}

/// Copy the requested attributes from `source` to `dest`.
///
/// Security-descriptor components (`owner`/`dacl`/`sacl`, and the
/// inherited-ACE-stripped `daclni`/`saclni` variants) are copied via
/// `kacs_get_sd` / `kacs_set_sd`; `daclni`/`saclni` additionally run the
/// fetched ACL through `strip_inherited_aces` so the destination's parent
/// supplies its own inheritance. The full-ACL request wins if both a full and
/// a no-inherited variant are set. SD copy failures are always fatal.
///
/// Both names are resolved exactly once, up front, into [`FileHandle`]s; every
/// step below then works through those handles, so nothing here can be
/// redirected by a path swap after the data copy has finished.
pub fn copy_attributes(source: &Path, dest: &Path, attributes: &Attributes) -> PreserveResult<()> {
    let context = format!("{} -> {}", source.quote(), dest.quote());
    let source = FileHandle::open(source, &context)?;
    let dest = FileHandle::open(dest, &context)?;
    let source_metadata = source.metadata();

    let want_owner = matches!(attributes.owner, Preserve::Yes { .. });
    let want_dacl = matches!(attributes.dacl, Preserve::Yes { .. });
    let want_daclni = matches!(attributes.daclni, Preserve::Yes { .. });
    let want_sacl = matches!(attributes.sacl, Preserve::Yes { .. });
    let want_saclni = matches!(attributes.saclni, Preserve::Yes { .. });

    let mut sd_info = SecInfo::empty();
    if want_owner {
        sd_info |= SecInfo::OWNER;
    }
    if want_dacl || want_daclni {
        sd_info |= SecInfo::DACL;
    }
    if want_sacl || want_saclni {
        sd_info |= SecInfo::SACL;
    }
    if !sd_info.is_empty() {
        // Each side is read and written through its own handle, so neither
        // follows a symlink (see `FileHandle::set_sd`). This used to key the
        // `AT_SYMLINK_NOFOLLOW` flag for *both* sides off `dest.is_symlink()`,
        // on the reasoning that `cp -a` implies `-d` and so the two ends match;
        // asking each end about itself is the same answer without the coupling.
        let sd = source.get_sd(sd_info).map_err(|e| {
            PreserveError::Other(format!("kacs_get_sd({}): {e}", source.path.quote()))
        })?;

        // Strip inherited ACEs from any ACL requested only in its
        // no-inherited form. The full-ACL request wins if both are set.
        let mut strip_info = SecInfo::empty();
        if want_daclni && !want_dacl {
            strip_info |= SecInfo::DACL;
        }
        if want_saclni && !want_sacl {
            strip_info |= SecInfo::SACL;
        }
        let sd = if !strip_info.is_empty() {
            strip_inherited(sd.as_bytes(), strip_info)
                .map_err(|e| PreserveError::Other(format!("strip_inherited: {e}")))?
        } else {
            sd
        };

        dest.set_sd(sd_info, &sd).map_err(|e| {
            PreserveError::Other(format!("kacs_set_sd({}): {e}", dest.path.quote()))
        })?;
    }

    // Executable-ness. Done after the SD copy so the dest's SD (which grants
    // WRITE_DAC) is in place for the chmod-equivalent setattr KACS gates.
    handle_preserve(attributes.exec, || -> PreserveResult<()> {
        #[cfg(unix)]
        {
            apply_exec(source_metadata, &dest)?;
        }
        Ok(())
    })?;

    // `--preserve=security` copies the `security.peios.*` xattr namespace
    // EXCEPT `security.peios.sd` (preserved via owner/dacl/sacl above).
    if matches!(attributes.security, Preserve::Yes { .. }) {
        #[cfg(all(unix, not(target_os = "android")))]
        copy_security_xattrs(&source, &dest)?;
    }

    handle_preserve(attributes.timestamps, || -> PreserveResult<()> {
        let atime = FileTime::from_last_access_time(source_metadata);
        let mtime = FileTime::from_last_modification_time(source_metadata);
        dest.set_times(atime, mtime)?;
        Ok(())
    })?;

    handle_preserve(attributes.xattrs, || -> PreserveResult<()> {
        #[cfg(all(unix, not(target_os = "android")))]
        {
            copy_extended_attrs(&source, &dest)?;
        }
        Ok(())
    })?;

    Ok(())
}

/// Returns `true` if `path`'s DACL is *protected* (`SE_DACL_PROTECTED`) —
/// i.e. inheritance was deliberately broken on it.
///
/// `mv` uses this to decide whether to warn before a cross-filesystem move:
/// such a move creates a new inode whose security descriptor is re-inherited
/// from the destination directory, so a protected DACL would be silently
/// lost. Any failure to read or parse the SD returns `false` (no warning).
pub fn dacl_is_protected(path: &Path) -> bool {
    let Ok(sd) = file::get_sd(None, path, SecInfo::DACL, 0) else {
        return false;
    };
    match SdView::parse(sd.as_bytes()) {
        Ok(view) => view.control().contains(Control::DACL_PROTECTED),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    /// GHSA-8r5f-98ww-c4c5 / GHSA-p9fh-vm43-9xxc. Once the destination name has
    /// been resolved, replacing it with a symlink must not redirect the `chmod`
    /// that the attribute phase performs: `chmod(2)` follows symlinks, so the
    /// old path form handed an attacker the destination's mode on an arbitrary
    /// file the copying process could chmod.
    #[test]
    fn chmod_through_a_handle_cannot_be_redirected_by_a_swap() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("dest");
        let victim = dir.path().join("victim");
        File::create(&dest).unwrap();
        File::create(&victim).unwrap();
        fs::set_permissions(&dest, Permissions::from_mode(0o444)).unwrap();
        fs::set_permissions(&victim, Permissions::from_mode(0o444)).unwrap();

        let handle = FileHandle::open(&dest, "src -> dest").unwrap();

        // The attacker wins the window: `dest` now names a symlink to `victim`.
        let moved = dir.path().join("moved");
        fs::rename(&dest, &moved).unwrap();
        symlink(&victim, &dest).unwrap();

        handle.set_mode(0o666).unwrap();

        assert_eq!(
            fs::metadata(&victim).unwrap().permissions().mode() & 0o777,
            0o444,
            "the chmod followed the symlink swapped in at the destination"
        );
        assert_eq!(
            fs::metadata(&moved).unwrap().permissions().mode() & 0o777,
            0o666,
            "the chmod did not reach the pinned inode"
        );
    }

    /// The same for extended attributes, which carry `security.capability` and
    /// the `security.peios.*` namespace.
    #[test]
    fn xattrs_through_a_handle_cannot_be_redirected_by_a_swap() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("dest");
        let victim = dir.path().join("victim");
        File::create(&dest).unwrap();
        File::create(&victim).unwrap();

        let handle = FileHandle::open(&dest, "src -> dest").unwrap();
        let name = OsString::from("user.peios_preserve_pin_test");
        if handle.set_xattr(&name, b"pinned").is_err() {
            // Filesystem without user extended attributes; nothing to test.
            return;
        }

        let moved = dir.path().join("moved");
        fs::rename(&dest, &moved).unwrap();
        symlink(&victim, &dest).unwrap();

        handle.set_xattr(&name, b"still pinned").unwrap();

        assert_eq!(
            xattr::get(&victim, &name).unwrap(),
            None,
            "the setxattr landed on the swapped-in symlink's target"
        );
        assert_eq!(
            handle.get_xattr(&name).unwrap().as_deref(),
            Some(&b"still pinned"[..])
        );
        assert_eq!(
            xattr::get(&moved, &name).unwrap().as_deref(),
            Some(&b"still pinned"[..])
        );
    }

    /// A symlink end is never opened, and gets no `chmod` at all — the path
    /// form would follow it onto the target.
    #[test]
    fn a_symlink_end_is_not_opened_and_is_never_chmodded() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        File::create(&target).unwrap();
        fs::set_permissions(&target, Permissions::from_mode(0o444)).unwrap();
        symlink(&target, &link).unwrap();

        let handle = FileHandle::open(&link, "src -> link").unwrap();
        assert!(handle.file.is_none());
        assert!(handle.is_symlink());

        handle.set_mode(0o777).unwrap();
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o444
        );
    }

    /// A read-only destination is still openable, so the widen/restore that
    /// `setxattr` needs keeps working through the descriptor.
    #[test]
    fn a_read_only_regular_file_is_still_pinned() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("dest");
        File::create(&dest).unwrap();
        fs::set_permissions(&dest, Permissions::from_mode(0o444)).unwrap();

        let handle = FileHandle::open(&dest, "src -> dest").unwrap();
        assert!(handle.file.is_some());
        assert_eq!(handle.current_mode().unwrap() & 0o777, 0o444);

        handle.set_mode(0o444 | 0o222).unwrap();
        assert_eq!(handle.current_mode().unwrap() & 0o777, 0o666);
        handle
            .set_mode(handle.current_mode().unwrap() & !0o222)
            .unwrap();
        assert_eq!(handle.current_mode().unwrap() & 0o777, 0o444);
    }

    /// A directory destination is opened read-only: it cannot be opened for
    /// writing, and `fsetxattr` checks write permission on the inode rather
    /// than the open mode. Upstream 6372fd3b2 makes the same point.
    #[test]
    fn a_directory_end_is_pinned_read_only() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();

        let handle = FileHandle::open(&sub, "src -> sub").unwrap();
        assert!(handle.file.is_some());
        assert!(handle.metadata().is_dir());

        let name = OsString::from("user.peios_preserve_dir_test");
        if handle.set_xattr(&name, b"dirvalue").is_ok() {
            assert_eq!(
                handle.get_xattr(&name).unwrap().as_deref(),
                Some(&b"dirvalue"[..])
            );
        }
    }

    #[test]
    fn parse_iter_unions_attributes() {
        let attrs = Attributes::parse_iter(["timestamps", "dacl"].into_iter()).unwrap();
        assert!(matches!(attrs.timestamps, Preserve::Yes { required: true }));
        assert!(matches!(attrs.dacl, Preserve::Yes { required: true }));
        assert!(matches!(attrs.owner, Preserve::No { explicit: false }));
    }

    #[test]
    fn parse_iter_rejects_unknown_attribute() {
        assert!(Attributes::parse_iter(["bogus"].into_iter()).is_err());
    }

    #[test]
    fn xattr_alias_maps_to_xattrs() {
        let attrs = Attributes::parse_iter(["xattr"].into_iter()).unwrap();
        assert!(matches!(attrs.xattrs, Preserve::Yes { .. }));
    }

    #[test]
    fn diff_flips_requested_fields_to_explicit_no() {
        let diffed = Attributes::ALL.diff(&Attributes::SD);
        assert!(matches!(diffed.owner, Preserve::No { explicit: true }));
        assert!(matches!(diffed.dacl, Preserve::No { explicit: true }));
        assert!(matches!(diffed.sacl, Preserve::No { explicit: true }));
        // Untouched fields keep their original value.
        assert!(matches!(
            diffed.timestamps,
            Preserve::Yes { required: true }
        ));
    }

    #[test]
    fn sd_explicit_uses_no_inherit_variants() {
        assert!(matches!(
            Attributes::SD_EXPLICIT.daclni,
            Preserve::Yes { required: true }
        ));
        assert!(matches!(
            Attributes::SD_EXPLICIT.dacl,
            Preserve::No { explicit: false }
        ));
    }

    #[test]
    fn exec_parses_as_required_when_listed() {
        let attrs = Attributes::parse_iter(["exec"].into_iter()).unwrap();
        assert!(matches!(attrs.exec, Preserve::Yes { required: true }));
    }

    #[test]
    fn all_and_default_include_exec_required() {
        assert!(matches!(
            Attributes::ALL.exec,
            Preserve::Yes { required: true }
        ));
        assert!(matches!(
            Attributes::DEFAULT.exec,
            Preserve::Yes { required: true }
        ));
    }

    #[test]
    fn implicit_preserves_exec_best_effort_and_nothing_else() {
        assert!(matches!(
            Attributes::IMPLICIT.exec,
            Preserve::Yes { required: false }
        ));
        assert!(matches!(
            Attributes::IMPLICIT.timestamps,
            Preserve::No { .. }
        ));
        assert!(matches!(Attributes::IMPLICIT.owner, Preserve::No { .. }));
        // NONE (the explicit-`--preserve` baseline) preserves exec too: nothing.
        assert!(matches!(Attributes::NONE.exec, Preserve::No { .. }));
    }

    #[test]
    fn no_preserve_can_drop_exec() {
        // `--no-preserve=exec` flips exec off via the standard diff machinery.
        let only_exec = Attributes {
            exec: Preserve::Yes { required: true },
            ..Attributes::NONE
        };
        let diffed = Attributes::IMPLICIT.diff(&only_exec);
        assert!(matches!(diffed.exec, Preserve::No { explicit: true }));
    }
}
