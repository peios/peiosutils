// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! The Peios SD-creation flag group.
//!
//! A shared clap argument group + parser for the object-creation commands
//! (`mkdir`, `mkfifo`, `mknod`, `touch`). It replaces the POSIX `-m` / umask
//! permission surface with flags that specify a newly created object's
//! Security Descriptor directly. See `peios/sd-creation-design.md`.
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
//! optional [`CreatorSd`]; the command creates the object, then
//! [`CreatorSd::apply_to`] writes the descriptor with `libp_sd::set_sd`.
//!
//! Applying is always post-create. The atomic `kacs_open` create path is
//! unusable for restrictive descriptors — §11.2's post-create strict
//! AccessCheck rolls back a create whose new SD does not self-grant the
//! creator a data right (`--no-inherit` being the obvious case). See the
//! design doc.

use std::path::Path;

use clap::{Arg, ArgAction, ArgMatches};
use libp_sd::{
    AceBuilder, AclBuilder, SdBuilder, SdTarget, SecurityDescriptor, SecurityInfo, Sid,
    WellKnownSid,
    consts::SE_DACL_PROTECTED,
    raw::FDCWD,
    sddl,
};

use crate::error::{UResult, USimpleError};

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

/// Mandatory-label policy bit `SYSTEM_MANDATORY_LABEL_NO_WRITE_UP`
/// (MS-DTYP §2.4.4.13): a subject below the object's integrity level
/// cannot write to it. This is the policy `--label` applies — the
/// standard one for files and directories.
const LABEL_NO_WRITE_UP: u32 = 0x0000_0001;

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
/// with the `SecurityInfo` mask naming which components it carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatorSd {
    blob: Vec<u8>,
    info: SecurityInfo,
}

impl CreatorSd {
    /// Apply the descriptor to a just-created object at `path` via
    /// `libp_sd::set_sd`.
    ///
    /// `path` must already exist — this is the create-then-set step. On
    /// failure the caller is expected to unlink the half-secured object.
    pub fn apply_to(&self, path: &Path) -> UResult<()> {
        let path_str = path.to_str().ok_or_else(|| {
            USimpleError::new(
                1,
                format!(
                    "{}: cannot apply a security descriptor — path is not valid UTF-8",
                    path.display()
                ),
            )
        })?;
        let target = SdTarget::path(path_str);
        libp_sd::set_sd(&target, self.info, &self.blob)
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
    let builder =
        sddl::parse(sddl_str).map_err(|e| USimpleError::new(1, format!("invalid SDDL: {e}")))?;
    let blob = builder
        .build()
        .map_err(|e| USimpleError::new(1, format!("invalid SDDL: {e}")))?;
    let info = info_from_blob(&blob, false)?;
    Ok(CreatorSd { blob, info })
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
        builder = builder.owner(parse_sid_arg("--owner", o)?);
    }
    if let Some(g) = group {
        builder = builder.group(parse_sid_arg("--group", g)?);
    }
    if no_inherit {
        // Empty DACL + SE_DACL_PROTECTED: no ACEs and no parent
        // inheritance, so the object is reachable only via its owner's
        // implicit READ_CONTROL / WRITE_DAC.
        builder = builder.dacl(AclBuilder::new()).control(SE_DACL_PROTECTED);
    }
    if let Some(level) = label {
        builder = builder.sacl(AclBuilder::new().ace(AceBuilder::mandatory_label(
            label_sid(level).to_sid(),
            LABEL_NO_WRITE_UP,
        )));
    }

    let blob = builder
        .build()
        .map_err(|e| USimpleError::new(1, format!("building security descriptor: {e}")))?;
    // The shortcuts never build a full audit SACL — the only SACL they
    // can produce is the `--label` mandatory-label ACE, so SACL presence
    // maps to LABEL_SECURITY_INFORMATION.
    let info = info_from_blob(&blob, true)?;
    Ok(Some(CreatorSd { blob, info }))
}

/// Resolve a `--owner` / `--group` SID argument, prefixing parse errors
/// with the offending flag name.
fn parse_sid_arg(flag: &str, value: &str) -> UResult<Sid> {
    sddl::parse_sid(value).map_err(|e| USimpleError::new(1, format!("{flag}: {e}")))
}

/// Map a validated `--label` level name to its integrity-level SID.
fn label_sid(level: &str) -> WellKnownSid {
    match level {
        "untrusted" => WellKnownSid::UntrustedIl,
        "low" => WellKnownSid::LowIl,
        "medium" => WellKnownSid::MediumIl,
        "medium-plus" => WellKnownSid::MediumPlusIl,
        "high" => WellKnownSid::HighIl,
        "system" => WellKnownSid::SystemIl,
        "protected" => WellKnownSid::ProtectedProcessIl,
        // clap's value_parser restricts --label to LABEL_LEVELS.
        other => unreachable!("unvalidated --label level: {other}"),
    }
}

/// Derive the `SecurityInfo` component mask from a built SD blob.
///
/// `sacl_is_label` selects how a present SACL is reported: the `--label`
/// shortcut produces a label-only SACL (`LABEL_SECURITY_INFORMATION`),
/// while an `--sddl` string with an `S:` section is a full SACL
/// (`SACL_SECURITY_INFORMATION`). The two cannot be combined — `SecurityInfo`
/// rejects `SACL | LABEL`.
fn info_from_blob(blob: &[u8], sacl_is_label: bool) -> UResult<SecurityInfo> {
    let sd = SecurityDescriptor::parse(blob).map_err(|e| {
        USimpleError::new(
            1,
            format!("internal error: built an unparseable security descriptor: {e:?}"),
        )
    })?;
    let mut info = SecurityInfo::none();
    if sd.owner_ref().is_some() {
        info = info.with_owner();
    }
    if sd.group_ref().is_some() {
        info = info.with_group();
    }
    if sd.dacl().is_some() {
        info = info.with_dacl();
    }
    if sd.sacl().is_some() {
        info = if sacl_is_label {
            info.with_label()
        } else {
            info.with_sacl()
        };
    }
    Ok(info)
}

/// A file's security-descriptor facts that a listing displays: the
/// owner SID and whether the DACL is inheritance-protected.
#[derive(Clone, Debug, Default)]
pub struct SdDisplay {
    /// Owner SID rendered as an `S-1-…` string. `None` when the
    /// descriptor could not be read or carries no owner.
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
    let Some(path_str) = path.to_str() else {
        return SdDisplay::default();
    };
    let target = SdTarget::Path {
        dirfd: FDCWD,
        path: path_str,
        no_follow_symlinks: !follow_symlinks,
    };
    let Ok(blob) = libp_sd::get_sd(&target, SecurityInfo::owner().with_dacl()) else {
        return SdDisplay::default();
    };
    let Ok(sd) = SecurityDescriptor::parse(&blob) else {
        return SdDisplay::default();
    };
    SdDisplay {
        owner: sd.owner_ref().map(|sid| sid.to_string()),
        protected_dacl: sd.control & SE_DACL_PROTECTED != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Command;
    use libp_sd::consts::{
        DACL_SECURITY_INFORMATION, LABEL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
        SE_DACL_PRESENT,
    };

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
        assert_eq!(creator.info.bits(), DACL_SECURITY_INFORMATION);
        let sd = SecurityDescriptor::parse(&creator.blob).unwrap();
        assert!(sd.control & SE_DACL_PRESENT != 0);
        assert!(sd.control & SE_DACL_PROTECTED != 0);
    }

    #[test]
    fn malformed_sddl_string_is_rejected() {
        assert!(creator_sd_from_sddl("not sddl at all").is_err());
    }

    #[test]
    fn owner_alias_sets_owner_component() {
        let creator = parse(&["--owner", "BA"]).unwrap().unwrap();
        assert_eq!(creator.info.bits(), OWNER_SECURITY_INFORMATION);
    }

    #[test]
    fn no_inherit_builds_empty_protected_dacl() {
        let creator = parse(&["--no-inherit"]).unwrap().unwrap();
        assert_eq!(creator.info.bits(), DACL_SECURITY_INFORMATION);
        let sd = SecurityDescriptor::parse(&creator.blob).unwrap();
        assert!(sd.control & SE_DACL_PRESENT != 0);
        assert!(sd.control & SE_DACL_PROTECTED != 0);
    }

    #[test]
    fn label_maps_to_label_component() {
        let creator = parse(&["--label", "high"]).unwrap().unwrap();
        assert_eq!(creator.info.bits(), LABEL_SECURITY_INFORMATION);
    }

    #[test]
    fn owner_and_label_combine() {
        let creator = parse(&["--owner", "BA", "--label", "medium"])
            .unwrap()
            .unwrap();
        assert_eq!(
            creator.info.bits(),
            OWNER_SECURITY_INFORMATION | LABEL_SECURITY_INFORMATION
        );
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
            let _ = label_sid(level);
        }
    }
}
