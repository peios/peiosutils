// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! The Peios SD-creation flag group.
//!
//! A shared clap argument group + parser for the object-creation commands
//! (`mkdir`, `mkfifo`, `mknod`, `touch`). It replaces the POSIX `-m` / umask
//! permission surface with flags that specify a newly created object's
//! Security Descriptor directly.
//!
//! | Flag | Argument | Effect |
//! |---|---|---|
//! | `--sddl` | SDDL string | Full creator security descriptor. |
//! | `--owner` | SID | Owner SID. |
//! | `--group` | SID | Group SID. |
//! | `--no-inherit` | — | Empty, inheritance-protected DACL. |
//! | `--label` | level | Mandatory integrity label. |
//!
//! `--sddl` is mutually exclusive with the four shortcuts (enforced by
//! clap). [`creator_sd_from_matches`] turns parsed [`ArgMatches`] into an
//! optional [`CreatorSd`].
//!
//! A regular file or directory is created carrying its descriptor, in one
//! native create ([`CreatorSd::create_file`], [`CreatorSd::create_dir`]),
//! so it never exists with the descriptor it would have inherited
//! instead. The kernel grants the creating open what it asks for even
//! where the new descriptor would refuse it, so a restrictive descriptor,
//! `--no-inherit` included, creates as readily as any other. See "The
//! creator's access" in the kernel's KACS native-open reference,
//! <https://learn.peios.org/kernel/internals/kacs/file-security/kacs-native-open>.
//!
//! `--label` is the exception. A descriptor supplied at creation carries
//! a full SACL, which takes `SeSecurityPrivilege`, while setting a label
//! alone takes only `WRITE_OWNER`. So the label is set just after the
//! create, through the creating handle rather than by name. Until then
//! the new object has the label it inherited.
//!
//! A FIFO or device node cannot be made by a native create, so `mkfifo`
//! and `mknod` create the node and then apply the descriptor by name
//! ([`CreatorSd::apply_to_created`]).

use std::io;
use std::path::Path;

use clap::{Arg, ArgAction, ArgMatches};
use peios::file::{self, CreateOptions, Disposition, FileAccess, OpenFlags, OpenOptions, SecInfo};
use peios::security::{
    AclBuilder, Control, IntegrityLevel, LabelPolicy, SdBuilder, SdView, SecurityDescriptor, Sid,
    sddl,
};

use crate::error::{UResult, USimpleError};
use crate::sid_render::SidStyle;

/// `AT_SYMLINK_NOFOLLOW` for the `at_flags` argument of `get_sd`/`set_sd`.
const AT_SYMLINK_NOFOLLOW: i32 = 0x100;

/// clap arg id — `--sddl`.
pub const OPT_SDDL: &str = "sdopt_sddl";
/// clap arg id — `--owner`.
pub const OPT_OWNER: &str = "sdopt_owner";
/// clap arg id — `--group`.
pub const OPT_GROUP: &str = "sdopt_group";
/// clap arg id — `--no-inherit`.
pub const OPT_NO_INHERIT: &str = "sdopt_no_inherit";
/// clap arg id — `--label`.
pub const OPT_LABEL: &str = "sdopt_label";

/// Integrity levels accepted by `--label`, lowest to highest.
const LABEL_LEVELS: [&str; 7] = [
    "untrusted",
    "low",
    "medium",
    "medium-plus",
    "high",
    "system",
    "protected",
];

/// The five `--sd*` arguments, ready to hand to `clap::Command::args`.
///
/// `--sddl` is declared `conflicts_with` the four shortcuts, so clap
/// rejects the combination at parse time — [`creator_sd_from_matches`]
/// never has to revalidate it.
pub fn args() -> [Arg; 5] {
    [
        Arg::new(OPT_SDDL)
            .long("sddl")
            .value_name("SDDL")
            .help("create with this security descriptor, given as an SDDL string")
            .conflicts_with_all([OPT_OWNER, OPT_GROUP, OPT_NO_INHERIT, OPT_LABEL]),
        Arg::new(OPT_OWNER)
            .long("owner")
            .value_name("SID")
            .help("set the owner SID (an S-1-… literal or a well-known alias such as BA)"),
        Arg::new(OPT_GROUP)
            .long("group")
            .value_name("SID")
            .help("set the group SID (an S-1-… literal or a well-known alias)"),
        Arg::new(OPT_NO_INHERIT)
            .long("no-inherit")
            .action(ArgAction::SetTrue)
            .help("do not inherit ACEs from the parent; lock the object to its owner"),
        Arg::new(OPT_LABEL)
            .long("label")
            .value_name("LEVEL")
            .value_parser(LABEL_LEVELS)
            .help("set the mandatory integrity label of the object"),
    ]
}

/// A creator Security Descriptor parsed from the `--sd*` flags, paired
/// with the `SecInfo` mask naming which components it carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatorSd {
    sd: SecurityDescriptor,
    info: SecInfo,
    /// What a native create is given: `sd` without a `--label` SACL, or
    /// `None` where the label is all there is.
    creation: Option<SecurityDescriptor>,
}

impl CreatorSd {
    /// Create the directory `path` carrying this descriptor.
    ///
    /// Fails with `EEXIST` if something is already there, a symlink
    /// included: a link at `path` is never followed.
    pub fn create_dir(&self, path: &Path) -> io::Result<()> {
        self.create(path, CreateOptions::DIRECTORY, true)
    }

    /// Create the empty regular file `path` carrying this descriptor.
    ///
    /// Fails with `EEXIST` if something is already there, a symlink
    /// included: a link at `path` is never followed.
    pub fn create_file(&self, path: &Path) -> io::Result<()> {
        self.create(path, CreateOptions::empty(), false)
    }

    fn create(&self, path: &Path, options: CreateOptions, directory: bool) -> io::Result<()> {
        let label = self.info.contains(SecInfo::LABEL);
        // A control handle: nothing is read or written through it. The
        // label write takes WRITE_OWNER, which the creating open is
        // granted because it asked.
        let mut access = FileAccess::READ_ATTRIBUTES | FileAccess::SYNCHRONIZE;
        if label {
            access |= FileAccess::WRITE_OWNER;
        }
        let mut open = OpenOptions::new();
        open.desired_access(access)
            .disposition(Disposition::Create)
            .options(options)
            .flags(OpenFlags::SYMLINK_NOFOLLOW);
        if let Some(creation) = &self.creation {
            open.creator_sd(creation);
        }
        let (handle, _) = open.create(None, path).map_err(io::Error::from)?;
        if label && let Err(e) = handle.fd_set_sd(SecInfo::LABEL, &self.sd) {
            drop(handle);
            let _ = if directory {
                std::fs::remove_dir(path)
            } else {
                std::fs::remove_file(path)
            };
            let e = io::Error::from(e);
            return Err(io::Error::new(
                e.kind(),
                format!("setting the integrity label: {e}"),
            ));
        }
        Ok(())
    }

    /// Apply the descriptor to an existing object at `path` via
    /// `peios::file::set_sd`, following a symlink there.
    ///
    /// For an object this process just created, use
    /// [`CreatorSd::create_file`] or [`CreatorSd::create_dir`] instead, or
    /// [`CreatorSd::apply_to_created`] where no native create can make it.
    pub fn apply_to(&self, path: &Path) -> UResult<()> {
        self.set(path, 0)
    }

    /// Apply the descriptor to an object this process just created at
    /// `path`, without following a symlink at the final component.
    ///
    /// The create-then-set step is a TOCTOU window: `mkfifo` and `mknod`
    /// create the node, then name it a second time to stamp the
    /// descriptor on it. In an attacker-writable directory the object can
    /// be replaced by a symlink in between, and a following `set_sd` would
    /// write the caller's owner/DACL/label onto whatever the link points
    /// at — the same shape as CVE-2026-35352 (`mkfifo`'s path-based chmod),
    /// but writing access control rather than mode bits. `AT_SYMLINK_NOFOLLOW`
    /// keeps the write on the object at that name.
    ///
    /// This is for a *just-created* object, which can never legitimately be
    /// a symlink. Use [`CreatorSd::apply_to`] where the caller means to
    /// follow (e.g. stamping a file the user named).
    pub fn apply_to_created(&self, path: &Path) -> UResult<()> {
        self.set(path, AT_SYMLINK_NOFOLLOW)
    }

    fn set(&self, path: &Path, at_flags: i32) -> UResult<()> {
        file::set_sd(None, path, self.info, &self.sd, at_flags)
            .map_err(|e| USimpleError::new(1, format!("setting security descriptor: {e}")))
    }
}

/// Parse a single SDDL string into a [`CreatorSd`].
///
/// Backs the `--sddl` flag, and is also how a command supplies a
/// built-in default creator descriptor — `nohup` secures the `nohup.out`
/// it creates this way.
///
/// # Errors
///
/// A malformed SDDL string, or one that fails to serialize, yields an
/// error.
pub fn creator_sd_from_sddl(sddl_str: &str) -> UResult<CreatorSd> {
    let sd =
        sddl::parse(sddl_str).map_err(|e| USimpleError::new(1, format!("invalid SDDL: {e}")))?;
    let info = info_from_blob(sd.as_bytes(), false)?;
    // An `S:` section here is a full SACL, which takes the same privilege
    // at creation as set afterwards, so it is created with the rest.
    let creation = Some(sd.clone());
    Ok(CreatorSd { sd, info, creation })
}

/// Parse the `--sd*` flags from `matches` into an optional [`CreatorSd`].
///
/// Returns `Ok(None)` when no SD flag was supplied — the command should
/// then create the object with plain kernel inheritance.
///
/// # Errors
///
/// A malformed `--sddl` string, an unparseable `--owner` / `--group` SID,
/// or a descriptor that fails to serialize yields an error. `--sddl`
/// versus the shortcuts is enforced by clap, not here.
pub fn creator_sd_from_matches(matches: &ArgMatches) -> UResult<Option<CreatorSd>> {
    if let Some(sddl_str) = matches.get_one::<String>(OPT_SDDL) {
        return Ok(Some(creator_sd_from_sddl(sddl_str)?));
    }

    let owner = matches.get_one::<String>(OPT_OWNER);
    let group = matches.get_one::<String>(OPT_GROUP);
    let no_inherit = matches.get_flag(OPT_NO_INHERIT);
    let label = matches.get_one::<String>(OPT_LABEL);

    if owner.is_none() && group.is_none() && !no_inherit && label.is_none() {
        return Ok(None);
    }

    let mut builder = SdBuilder::new();
    if let Some(o) = owner {
        builder.owner(&parse_sid_arg("--owner", o)?);
    }
    if let Some(g) = group {
        builder.group(&parse_sid_arg("--group", g)?);
    }
    if no_inherit {
        // Empty DACL + DACL_PROTECTED: no ACEs and no parent
        // inheritance, so the object is reachable only via its owner's
        // implicit READ_CONTROL / WRITE_DAC.
        let dacl = AclBuilder::new()
            .build()
            .map_err(|e| USimpleError::new(1, format!("building security descriptor: {e}")))?;
        builder
            .dacl(&dacl)
            .control(Control::DACL_PROTECTED, Control::empty());
    }
    let build = |builder: &SdBuilder| {
        builder
            .build()
            .map_err(|e| USimpleError::new(1, format!("building security descriptor: {e}")))
    };
    // The descriptor a native create is given has no label: see the
    // module documentation.
    let creation = (owner.is_some() || group.is_some() || no_inherit)
        .then(|| build(&builder))
        .transpose()?;
    if let Some(level) = label {
        let sacl = AclBuilder::new()
            .label(label_rid(level), LabelPolicy::NO_WRITE_UP)
            .build()
            .map_err(|e| USimpleError::new(1, format!("building security descriptor: {e}")))?;
        builder.sacl(&sacl);
    }

    let sd = build(&builder)?;
    // The shortcuts never build a full audit SACL — the only SACL they
    // can produce is the `--label` mandatory-label ACE, so SACL presence
    // maps to LABEL (rather than full SACL) security information.
    let info = info_from_blob(sd.as_bytes(), true)?;
    Ok(Some(CreatorSd { sd, info, creation }))
}

/// Resolve a `--owner` / `--group` SID argument, prefixing parse errors
/// with the offending flag name.
///
/// Accepts both an `S-1-…` literal (via `Sid: FromStr`) and a well-known
/// SDDL alias such as `BA`. The peios `Sid` string parser only handles the
/// numeric `S-1-…` form, so alias resolution goes through the SDDL codec:
/// parsing `O:<alias>` yields an SD whose owner is the resolved SID.
fn parse_sid_arg(flag: &str, value: &str) -> UResult<Sid> {
    if let Ok(sid) = value.parse::<Sid>() {
        return Ok(sid);
    }
    let err = || USimpleError::new(1, format!("{flag}: invalid SID {value:?}"));
    let owner_sd = sddl::parse(&format!("O:{value}")).map_err(|_| err())?;
    let view = SdView::parse(owner_sd.as_bytes()).map_err(|_| err())?;
    Ok(view.owner().ok_or_else(err)?.to_sid())
}

/// Map a validated `--label` level name to its integrity-level RID
/// (the RID of the `S-1-16-x` label SID).
fn label_rid(level: &str) -> u32 {
    let il = match level {
        "untrusted" => IntegrityLevel::UNTRUSTED,
        "low" => IntegrityLevel::LOW,
        "medium" => IntegrityLevel::MEDIUM,
        // `IntegrityLevel` exposes no named medium-plus / protected-process
        // constants, so use their standard RIDs (medium-plus = 0x2100,
        // protected-process = 0x5000) directly.
        "medium-plus" => IntegrityLevel(0x2100),
        "high" => IntegrityLevel::HIGH,
        "system" => IntegrityLevel::SYSTEM,
        "protected" => IntegrityLevel(0x5000),
        // clap's value_parser restricts --label to LABEL_LEVELS.
        other => unreachable!("unvalidated --label level: {other}"),
    };
    il.rid()
}

/// Derive the `SecInfo` component mask from a built SD blob.
///
/// `sacl_is_label` selects how a present SACL is reported: the `--label`
/// shortcut produces a label-only SACL (`SecInfo::LABEL`), while an
/// `--sddl` string with an `S:` section is a full SACL (`SecInfo::SACL`).
fn info_from_blob(blob: &[u8], sacl_is_label: bool) -> UResult<SecInfo> {
    let sd = SdView::parse(blob).map_err(|e| {
        USimpleError::new(
            1,
            format!("internal error: built an unparseable security descriptor: {e:?}"),
        )
    })?;
    let mut info = SecInfo::empty();
    if sd.owner().is_some() {
        info |= SecInfo::OWNER;
    }
    if sd.group().is_some() {
        info |= SecInfo::GROUP;
    }
    if sd.dacl().is_some() {
        info |= SecInfo::DACL;
    }
    if sd.sacl().is_some() {
        info |= if sacl_is_label {
            SecInfo::LABEL
        } else {
            SecInfo::SACL
        };
    }
    Ok(info)
}

/// A file's security-descriptor facts that a listing displays: the
/// owner SID and whether the DACL is inheritance-protected.
#[derive(Clone, Debug, Default)]
pub struct SdDisplay {
    /// The owner, rendered by [`crate::sid_render`] in [`SidStyle::Label`]
    /// style: a well-known name where one exists, the raw `S-1-…` form
    /// otherwise. `None` when the descriptor could not be read or carries no
    /// owner.
    ///
    /// `Label` rather than `Both` because this is a *column* — `Local System`
    /// is what a listing wants, not `Local System (S-1-5-18)`. `sd` and
    /// `token`, which report on one object at a time, default to `Both`.
    pub owner: Option<String>,
    /// True if the DACL is inheritance-protected (`SE_DACL_PROTECTED`):
    /// the file's access control is locked rather than tracking its
    /// parent directory. A freshly created file is not protected even
    /// though it carries non-inherited creator ACEs, so this — not mere
    /// ACE provenance — is the signal that an SD was set deliberately.
    pub protected_dacl: bool,
}

/// Read the owner SID and DACL-protection state of the object at `path`
/// via `libp_sd::get_sd`.
///
/// `follow_symlinks` selects whether a symlink's own descriptor or its
/// target's is read. Any failure — an unreadable object, a kernel
/// without the KACS SD syscalls, a malformed descriptor — yields
/// `SdDisplay::default()`, which a caller renders as `?`.
pub fn read_sd_display(path: &Path, follow_symlinks: bool) -> SdDisplay {
    let at_flags = if follow_symlinks {
        0
    } else {
        AT_SYMLINK_NOFOLLOW
    };
    let Ok(blob) = file::get_sd(None, path, SecInfo::OWNER | SecInfo::DACL, at_flags) else {
        return SdDisplay::default();
    };
    let Ok(sd) = SdView::parse(blob.as_bytes()) else {
        return SdDisplay::default();
    };
    SdDisplay {
        owner: sd
            .owner()
            .map(|sid| crate::sid_render::render(sid, SidStyle::Label)),
        protected_dacl: sd.control().contains(Control::DACL_PROTECTED),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Command;

    fn parse(argv: &[&str]) -> UResult<Option<CreatorSd>> {
        let matches = Command::new("test")
            .args(args())
            .try_get_matches_from(std::iter::once("test").chain(argv.iter().copied()))
            .expect("clap parse failed");
        creator_sd_from_matches(&matches)
    }

    #[test]
    fn no_flags_yields_none() {
        assert!(parse(&[]).unwrap().is_none());
    }

    #[test]
    fn sddl_string_parses_to_protected_dacl() {
        // nohup's built-in nohup.out descriptor: a protected DACL whose
        // only ACE grants the Owner Rights SID (S-1-3-4) full access.
        let creator = creator_sd_from_sddl("D:P(A;;FA;;;S-1-3-4)").unwrap();
        assert_eq!(creator.info, SecInfo::DACL);
        let sd = SdView::parse(creator.sd.as_bytes()).unwrap();
        assert!(sd.control().contains(Control::DACL_PRESENT));
        assert!(sd.control().contains(Control::DACL_PROTECTED));
    }

    #[test]
    fn malformed_sddl_string_is_rejected() {
        assert!(creator_sd_from_sddl("not sddl at all").is_err());
    }

    #[test]
    fn owner_alias_sets_owner_component() {
        let creator = parse(&["--owner", "BA"]).unwrap().unwrap();
        assert_eq!(creator.info, SecInfo::OWNER);
    }

    #[test]
    fn no_inherit_builds_empty_protected_dacl() {
        let creator = parse(&["--no-inherit"]).unwrap().unwrap();
        assert_eq!(creator.info, SecInfo::DACL);
        let sd = SdView::parse(creator.sd.as_bytes()).unwrap();
        assert!(sd.control().contains(Control::DACL_PRESENT));
        assert!(sd.control().contains(Control::DACL_PROTECTED));
    }

    #[test]
    fn label_maps_to_label_component() {
        let creator = parse(&["--label", "high"]).unwrap().unwrap();
        assert_eq!(creator.info, SecInfo::LABEL);
    }

    #[test]
    fn owner_and_label_combine() {
        let creator = parse(&["--owner", "BA", "--label", "medium"])
            .unwrap()
            .unwrap();
        assert_eq!(creator.info, SecInfo::OWNER | SecInfo::LABEL);
    }

    #[test]
    fn bad_owner_sid_is_rejected() {
        assert!(parse(&["--owner", "not-a-sid"]).is_err());
    }

    #[test]
    fn sddl_and_shortcut_conflict() {
        let result = Command::new("test")
            .args(args())
            .try_get_matches_from(["test", "--sddl", "O:BA", "--owner", "BU"]);
        assert!(result.is_err());
    }

    #[test]
    fn label_sid_covers_every_level() {
        for level in LABEL_LEVELS {
            let _ = label_rid(level);
        }
    }
}
