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
use std::io;
use std::path::Path;

use clap::ArgMatches;
use filetime::FileTime;
use peios::file::{self, SecInfo};
use peios::security::{Control, SdView, strip_inherited};

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
pub fn resolve(matches: &ArgMatches, options: &[(&str, PreserveOpt)]) -> PreserveResult<Attributes> {
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

/// Copy extended attributes (`user.*`, `trusted.*`, `system.*` — everything
/// outside the `security.` namespace) from `source` to `dest`.
#[cfg(all(unix, not(target_os = "android")))]
fn copy_extended_attrs(source: &Path, dest: &Path) -> PreserveResult<()> {
    // Security xattrs ride under `--preserve=security` (or under
    // owner/dacl/sacl for the SD itself), not here.
    copy_xattrs_filtered(source, dest, |name| {
        !name.as_encoded_bytes().starts_with(b"security.")
    })
}

/// Copy `security.*` xattrs from `source` to `dest`, excluding
/// `security.peios.sd` (which is preserved via the SD copy path).
#[cfg(all(unix, not(target_os = "android")))]
fn copy_security_xattrs(source: &Path, dest: &Path) -> PreserveResult<()> {
    copy_xattrs_filtered(source, dest, |name| {
        let bytes = name.as_encoded_bytes();
        bytes.starts_with(b"security.") && bytes != b"security.peios.sd"
    })
}

/// Walk `source`'s xattrs, copy those matching `keep` to `dest`. Temporarily
/// clears the readonly flag on `dest` if needed and restores it afterwards.
#[cfg(all(unix, not(target_os = "android")))]
fn copy_xattrs_filtered(
    source: &Path,
    dest: &Path,
    keep: impl Fn(&OsString) -> bool,
) -> PreserveResult<()> {
    use std::fs;

    let metadata = fs::symlink_metadata(dest)?;
    let mut perms = metadata.permissions();
    let was_readonly = perms.readonly();

    if was_readonly {
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        fs::set_permissions(dest, perms)?;
    }

    let result: io::Result<()> = (|| {
        for attr_name in xattr::list(source)? {
            if !keep(&attr_name) {
                continue;
            }
            if let Some(value) = xattr::get(source, &attr_name)? {
                xattr::set(dest, &attr_name, &value)?;
            }
        }
        Ok(())
    })();

    if was_readonly {
        let mut revert_perms = fs::symlink_metadata(dest)?.permissions();
        revert_perms.set_readonly(true);
        fs::set_permissions(dest, revert_perms)?;
    }

    result.map_err(|e| {
        PreserveError::IoContext(
            e,
            format!("failed to set extended attributes on {}", dest.quote()),
        )
    })?;

    Ok(())
}

/// Mirror the source's executable-ness onto `dest`: if the source has any
/// POSIX execute bit set, set all three on `dest`; otherwise clear them.
/// Read/write bits are left untouched (irrelevant under CAP_DAC_OVERRIDE).
/// Symlinks are skipped — a symlink's exec-ness is its target's, and most
/// platforms can't chmod the link itself.
#[cfg(unix)]
fn apply_exec(source_metadata: &std::fs::Metadata, dest: &Path) -> PreserveResult<()> {
    use std::os::unix::fs::PermissionsExt;
    if dest.is_symlink() {
        return Ok(());
    }
    let any_exec = source_metadata.permissions().mode() & 0o111 != 0;
    let mut perms = std::fs::symlink_metadata(dest)?.permissions();
    let mode = perms.mode();
    perms.set_mode((mode & !0o111) | if any_exec { 0o111 } else { 0 });
    std::fs::set_permissions(dest, perms)?;
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
pub fn copy_attributes(
    source: &Path,
    dest: &Path,
    attributes: &Attributes,
) -> PreserveResult<()> {
    use std::fs;

    let context = format!("{} -> {}", source.quote(), dest.quote());
    let source_metadata = fs::symlink_metadata(source)
        .map_err(|e| PreserveError::IoContext(e, context.clone()))?;

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
        let sd = file::get_sd(None, source, sd_info, 0)
            .map_err(|e| PreserveError::Other(format!("kacs_get_sd({}): {e}", source.quote())))?;

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

        file::set_sd(None, dest, sd_info, &sd, 0)
            .map_err(|e| PreserveError::Other(format!("kacs_set_sd({}): {e}", dest.quote())))?;
    }

    // Executable-ness. Done after the SD copy so the dest's SD (which grants
    // WRITE_DAC) is in place for the chmod-equivalent setattr KACS gates.
    handle_preserve(attributes.exec, || -> PreserveResult<()> {
        #[cfg(unix)]
        {
            apply_exec(&source_metadata, dest)?;
        }
        Ok(())
    })?;

    // `--preserve=security` copies the `security.peios.*` xattr namespace
    // EXCEPT `security.peios.sd` (preserved via owner/dacl/sacl above).
    if matches!(attributes.security, Preserve::Yes { .. }) {
        #[cfg(all(unix, not(target_os = "android")))]
        copy_security_xattrs(source, dest)?;
    }

    handle_preserve(attributes.timestamps, || -> PreserveResult<()> {
        let atime = FileTime::from_last_access_time(&source_metadata);
        let mtime = FileTime::from_last_modification_time(&source_metadata);
        if dest.is_symlink() {
            filetime::set_symlink_file_times(dest, atime, mtime)?;
        } else {
            filetime::set_file_times(dest, atime, mtime)?;
        }
        Ok(())
    })?;

    handle_preserve(attributes.xattrs, || -> PreserveResult<()> {
        #[cfg(all(unix, not(target_os = "android")))]
        {
            copy_extended_attrs(source, dest)?;
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
        assert!(matches!(diffed.timestamps, Preserve::Yes { required: true }));
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
        assert!(matches!(Attributes::ALL.exec, Preserve::Yes { required: true }));
        assert!(matches!(Attributes::DEFAULT.exec, Preserve::Yes { required: true }));
    }

    #[test]
    fn implicit_preserves_exec_best_effort_and_nothing_else() {
        assert!(matches!(Attributes::IMPLICIT.exec, Preserve::Yes { required: false }));
        assert!(matches!(Attributes::IMPLICIT.timestamps, Preserve::No { .. }));
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
