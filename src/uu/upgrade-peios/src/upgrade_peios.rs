// upgrade-peios ~ (peiosutils) — entry point.
//
// A Peios release is a package: the edition package
// (dev.peios.peios-experimental, later dev.peios.peios-pro and friends)
// whose version is the OS version and whose
// dependency closure is the base system. Moving a system to the next
// release is therefore a package upgrade — but not *only* a package
// upgrade. The package manager is deliberately weak: installing a package
// cannot change system policy, so the release's registry seeds (which
// services run, which policies apply) are not applied by peipkg. The
// edition ships them as data, /usr/share/peios/release.toml, and this tool
// is what acts on it.
//
// The edition package declares `alternate_upgrade`, so peipkg refuses to
// move it by name and holds it back from a bulk upgrade, pointing here.
// This tool is the alternate path: it drives peipkg with
// --bypass-alternate-upgrade, then reconciles the seeds. That flag is the
// whole of its privilege — upgrade-peios is a sequencer, not a broker; the
// caller's token governs every step, exactly as it would running the two
// commands by hand.
//
// Idempotent by construction: re-running after an interrupted upgrade
// re-stages the seeds and applies whatever is still queued; a system
// already current does nothing.

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};
use uucore::error::{UResult, USimpleError};

/// Where the release states what it asks of the system beyond its packages.
const RELEASE_FILE: &str = "usr/share/peios/release.toml";
/// Seed masters, shipped by packages; listing one in release.toml opts it in.
const MASTER_DIR: &str = "usr/share/regim";
/// The drain queue peinit's autorun applies on boot.
const AUTOAPPLY_DIR: &str = "lcl/policy/autoapply.d";
const AUTORUN_DIR: &str = "lcl/policy/autorun.d";
const DRAIN_SCRIPT: &str = "10-apply-seeds.sh";
const DRAIN: &str = "#!/bin/sh
# Placed by peiso / upgrade-peios. Apply the queued registry seeds, draining
# each after it applies (--once-delete). Run every boot by peinit; a no-op
# once the queues are empty.
#
# Two directories, in this order: autoapply.d is what this system applies
# wherever it is running, autoapply.live.d is what only a boot medium applies.
# An installed machine has no second directory, and `reg apply --dir` on one
# that is not there applies nothing and succeeds -- so this is the same script
# on both, which is what lets peiso and upgrade-peios write the same one.
set -e
/bin/reg apply --dir /lcl/policy/autoapply.d --once-delete
exec /bin/reg apply --dir /lcl/policy/autoapply.live.d --once-delete
";

pub type Result<T> = std::result::Result<T, Error>;

/// Exit codes: 1 usage, 2 no edition / release data, 3 peipkg failed,
/// 4 seed staging failed, 5 reg apply failed.
#[derive(Debug)]
pub enum Error {
    Usage(String),
    NoRelease(String),
    Peipkg {
        operation: &'static str,
        code: Option<i32>,
    },
    PeipkgQuery(String),
    Seeds(String),
    Apply(Option<i32>),
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 1,
            Self::NoRelease(_) => 2,
            Self::Peipkg { .. } | Self::PeipkgQuery(_) => 3,
            Self::Seeds(_) => 4,
            Self::Apply(_) => 5,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(m) | Self::NoRelease(m) | Self::PeipkgQuery(m) | Self::Seeds(m) => {
                f.write_str(m)
            }
            Self::Peipkg { operation, code } => {
                write!(f, "peipkg {operation} failed{}", exit_suffix(*code))
            }
            Self::Apply(code) => write!(f, "reg apply failed{}", exit_suffix(*code)),
        }
    }
}

fn exit_suffix(code: Option<i32>) -> String {
    code.map(|c| format!(" (exit {c})")).unwrap_or_default()
}

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match uu_app().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            let code = e.exit_code();
            e.print().ok();
            return if code == 0 {
                Ok(())
            } else {
                Err(USimpleError::new(code, ""))
            };
        }
    };
    match run(&matches) {
        Ok(()) => Ok(()),
        Err(err) => Err(USimpleError::new(err.exit_code(), err.to_string())),
    }
}

pub fn uu_app() -> ClapCommand {
    ClapCommand::new("upgrade-peios")
        .version(uucore::crate_version!())
        .about("Move this Peios to the next release of its edition")
        .long_about(
            "Upgrades the edition package through peipkg (the one package peipkg \
             will not move on its own), then applies the release's registry seeds. \
             Safe to re-run: an interrupted upgrade is completed, a current system \
             is left alone.",
        )
        .arg(
            Arg::new("on-reboot")
                .long("on-reboot")
                .action(ArgAction::SetTrue)
                .help("Stage the release's seeds for the next boot instead of applying them now"),
        )
        .arg(
            Arg::new("seeds-only")
                .long("seeds-only")
                .action(ArgAction::SetTrue)
                .help("Skip the package upgrade; only reconcile the installed release's seeds"),
        )
        .arg(
            Arg::new("yes")
                .long("yes")
                .short('y')
                .action(ArgAction::SetTrue)
                .help("Skip peipkg's confirmation prompt"),
        )
        .arg(
            Arg::new("root")
                .long("root")
                .value_name("DIR")
                .default_value("/")
                .help("Operate on the Peios rooted at DIR (implies --on-reboot unless DIR is /)"),
        )
}

fn run(m: &ArgMatches) -> Result<()> {
    let root = PathBuf::from(m.get_one::<String>("root").map_or("/", String::as_str));
    let live = root == Path::new("/");
    let on_reboot = m.get_flag("on-reboot") || !live;
    let mut out = io::stdout().lock();

    let edition = edition(&root)?;

    if !m.get_flag("seeds-only") {
        let installed = installed_packages(&root, live)?;
        let operation = edition_operation(&edition, &installed)?;
        match &operation {
            EditionOperation::Install { legacy, concrete } => {
                writeln!(out, "upgrade-peios: migrating {legacy} to {concrete}").ok();
            }
            EditionOperation::Upgrade { concrete } => {
                writeln!(out, "upgrade-peios: upgrading {concrete}").ok();
            }
        }
        let mut cmd = peipkg_command(&root, live, &operation);
        if m.get_flag("yes") {
            cmd.arg("--yes");
        }
        let status = cmd
            .status()
            .map_err(|e| Error::Usage(format!("cannot run peipkg: {e}")))?;
        if !status.success() {
            return Err(Error::Peipkg {
                operation: operation.verb(),
                code: status.code(),
            });
        }
    }

    let seeds = release_seeds(&root)?;
    let staged = stage_seeds(&root, &seeds)?;
    for name in &staged {
        writeln!(out, "upgrade-peios: staged seed {name}").ok();
    }

    if on_reboot {
        writeln!(
            out,
            "upgrade-peios: {} seed(s) queued; they apply on the next boot",
            staged.len()
        )
        .ok();
        return Ok(());
    }
    if staged.is_empty() {
        return Ok(());
    }
    writeln!(out, "upgrade-peios: applying {} seed(s)", staged.len()).ok();
    let status = Command::new("reg")
        .args([
            "apply",
            "--dir",
            &format!("/{AUTOAPPLY_DIR}"),
            "--once-delete",
            "--yes",
        ])
        .status()
        .map_err(|e| Error::Usage(format!("cannot run reg: {e}")))?;
    if !status.success() {
        return Err(Error::Apply(status.code()));
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
struct Edition {
    legacy: String,
    concrete: String,
}

/// The edition identities derived from os-release. Current packages use
/// `dev.peios.peios-<VARIANT_ID>`; `peios-<VARIANT_ID>` is retained only long
/// enough to migrate an installed pre-qualification package.
fn edition(root: &Path) -> Result<Edition> {
    let path = root.join("usr/lib/os-release");
    let text = fs::read_to_string(&path)
        .map_err(|e| Error::NoRelease(format!("{}: {e}", path.display())))?;
    let mut id = None;
    let mut variant = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("ID=") {
            id = Some(unquote(v));
        } else if let Some(v) = line.strip_prefix("VARIANT_ID=") {
            variant = Some(unquote(v));
        }
    }
    if id.as_deref() != Some("peios") {
        return Err(Error::NoRelease(format!(
            "{}: this is not a Peios system (ID is not peios)",
            path.display()
        )));
    }
    match variant {
        Some(v) if !v.is_empty() => {
            let legacy = format!("peios-{v}");
            Ok(Edition {
                concrete: format!("dev.peios.{legacy}"),
                legacy,
            })
        }
        _ => Err(Error::NoRelease(format!(
            "{}: no VARIANT_ID; cannot tell which edition is installed",
            path.display()
        ))),
    }
}

#[derive(serde::Deserialize)]
struct InstalledPackage {
    #[serde(rename = "Name")]
    name: String,
}

/// Read the package database through peipkg's stable JSON interface. Looking
/// at os-release alone cannot distinguish an old package from its qualified
/// successor because both intentionally write the same system identity.
fn installed_packages(root: &Path, live: bool) -> Result<HashSet<String>> {
    let mut cmd = Command::new("peipkg");
    add_root_args(&mut cmd, root, live);
    let output = cmd
        .args(["list", "--json"])
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| Error::PeipkgQuery(format!("cannot query installed packages: {e}")))?;
    if !output.status.success() {
        return Err(Error::PeipkgQuery(format!(
            "peipkg list failed{}",
            exit_suffix(output.status.code())
        )));
    }
    let packages: Vec<InstalledPackage> = serde_json::from_slice(&output.stdout)
        .map_err(|e| Error::PeipkgQuery(format!("peipkg list returned invalid JSON: {e}")))?;
    Ok(packages.into_iter().map(|p| p.name).collect())
}

#[derive(Debug, Eq, PartialEq)]
enum EditionOperation {
    Install { legacy: String, concrete: String },
    Upgrade { concrete: String },
}

impl EditionOperation {
    fn verb(&self) -> &'static str {
        match self {
            Self::Install { .. } => "install",
            Self::Upgrade { .. } => "upgrade",
        }
    }
}

/// Pick a concrete package operation. Peipkg intentionally never follows a
/// `provides` edge for a named upgrade, so the one-time rename is an install
/// of the qualified package; its bounded `replaces` declaration removes the
/// legacy package in the same transaction. Every subsequent run upgrades the
/// qualified concrete name normally.
fn edition_operation(edition: &Edition, installed: &HashSet<String>) -> Result<EditionOperation> {
    match (
        installed.contains(&edition.legacy),
        installed.contains(&edition.concrete),
    ) {
        (true, false) => Ok(EditionOperation::Install {
            legacy: edition.legacy.clone(),
            concrete: edition.concrete.clone(),
        }),
        (false, true) => Ok(EditionOperation::Upgrade {
            concrete: edition.concrete.clone(),
        }),
        (true, true) => Err(Error::NoRelease(format!(
            "both {} and {} are installed; refusing an ambiguous edition upgrade",
            edition.legacy, edition.concrete
        ))),
        (false, false) => Err(Error::NoRelease(format!(
            "neither {} nor {} is installed; os-release and the package database disagree",
            edition.legacy, edition.concrete
        ))),
    }
}

fn peipkg_command(root: &Path, live: bool, operation: &EditionOperation) -> Command {
    let mut cmd = Command::new("peipkg");
    add_root_args(&mut cmd, root, live);
    match operation {
        EditionOperation::Install { concrete, .. } => {
            cmd.arg("install").arg(concrete);
        }
        EditionOperation::Upgrade { concrete } => {
            cmd.arg("upgrade").arg(concrete);
        }
    }
    cmd.arg("--bypass-alternate-upgrade");
    cmd
}

fn add_root_args(cmd: &mut Command, root: &Path, live: bool) {
    if !live {
        cmd.arg("--root").arg(root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn edition() -> Edition {
        Edition {
            legacy: "peios-experimental".into(),
            concrete: "dev.peios.peios-experimental".into(),
        }
    }

    fn names(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn command_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(OsStr::to_string_lossy)
            .map(std::borrow::Cow::into_owned)
            .collect()
    }

    #[test]
    fn legacy_edition_is_migrated_by_installing_the_concrete_successor() {
        let operation = edition_operation(&edition(), &names(&["peios-experimental"])).unwrap();
        assert_eq!(
            operation,
            EditionOperation::Install {
                legacy: "peios-experimental".into(),
                concrete: "dev.peios.peios-experimental".into(),
            }
        );
        let command = peipkg_command(Path::new("/mnt/system"), false, &operation);
        assert_eq!(command.get_program(), OsStr::new("peipkg"));
        assert_eq!(
            command_args(&command),
            [
                "--root",
                "/mnt/system",
                "install",
                "dev.peios.peios-experimental",
                "--bypass-alternate-upgrade",
            ]
        );
    }

    #[test]
    fn qualified_edition_uses_a_concrete_named_upgrade() {
        let operation =
            edition_operation(&edition(), &names(&["dev.peios.peios-experimental"])).unwrap();
        assert_eq!(
            operation,
            EditionOperation::Upgrade {
                concrete: "dev.peios.peios-experimental".into(),
            }
        );
        let command = peipkg_command(Path::new("/"), true, &operation);
        assert_eq!(
            command_args(&command),
            [
                "upgrade",
                "dev.peios.peios-experimental",
                "--bypass-alternate-upgrade",
            ]
        );
    }

    #[test]
    fn ambiguous_or_missing_package_database_state_is_refused() {
        let both = names(&["peios-experimental", "dev.peios.peios-experimental"]);
        assert!(matches!(
            edition_operation(&edition(), &both),
            Err(Error::NoRelease(message)) if message.contains("both")
        ));
        assert!(matches!(
            edition_operation(&edition(), &HashSet::new()),
            Err(Error::NoRelease(message)) if message.contains("neither")
        ));
    }

    #[test]
    fn peipkg_json_names_are_decoded_without_coupling_to_other_fields() {
        let packages: Vec<InstalledPackage> = serde_json::from_slice(
            br#"[{"Name":"peios-experimental","Version":"2026.8-9","Architecture":"x86_64","Origin":"peios","orphaned":false}]"#,
        )
        .unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "peios-experimental");
    }
}

fn unquote(v: &str) -> String {
    v.trim().trim_matches('"').to_string()
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Release {
    #[serde(default)]
    registry: Registry,
}

/// The release's three seed lists. Only one of them is this tool's.
///
/// `deny_unknown_fields` on both, so a key a newer release adds stops an
/// older upgrade-peios loudly rather than being ignored — the whole
/// reason release.toml is read strictly is that silently dropping a list
/// produces a system missing exactly the policy the operator upgraded to
/// get.
///
/// The two it declines are declined for the same reason, from opposite
/// ends: an upgrade is neither making a boot medium nor making a machine.
/// `live_autoapply` belongs to an image being built, and
/// `install_autoapply` to a machine being installed — re-applying that
/// one here would resurrect a first-boot flow on a system that has been
/// running for a year.
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    #[serde(default)]
    autoapply: Vec<String>,
    #[serde(default, rename = "live_autoapply")]
    _live_autoapply: Vec<String>,
    #[serde(default, rename = "install_autoapply")]
    _install_autoapply: Vec<String>,
}

/// The seeds the installed release asks for, in release.toml order.
fn release_seeds(root: &Path) -> Result<Vec<String>> {
    let path = root.join(RELEASE_FILE);
    let text = fs::read_to_string(&path).map_err(|e| {
        Error::NoRelease(format!(
            "{}: {e} (does the edition ship release.toml?)",
            path.display()
        ))
    })?;
    let rel: Release =
        toml::from_str(&text).map_err(|e| Error::NoRelease(format!("{}: {e}", path.display())))?;
    Ok(rel.registry.autoapply)
}

/// Copies each named seed master into the autoapply queue and makes sure
/// the drain script is in place. Returns the staged names.
fn stage_seeds(root: &Path, names: &[String]) -> Result<Vec<String>> {
    let queue = root.join(AUTOAPPLY_DIR);
    fs::create_dir_all(&queue).map_err(|e| Error::Seeds(format!("{}: {e}", queue.display())))?;
    let mut staged = Vec::with_capacity(names.len());
    for name in names {
        if name.is_empty() || name.contains('/') || name.starts_with('.') {
            return Err(Error::Seeds(format!(
                "release.toml names an invalid seed {name:?}"
            )));
        }
        let src = root.join(MASTER_DIR).join(format!("{name}.reg"));
        let dst = queue.join(format!("{name}.reg"));
        let bytes = fs::read(&src).map_err(|e| {
            Error::Seeds(format!(
                "seed {name}: {}: {e} (is the package that ships it installed?)",
                src.display()
            ))
        })?;
        fs::write(&dst, bytes).map_err(|e| Error::Seeds(format!("{}: {e}", dst.display())))?;
        staged.push(name.clone());
    }
    let run_dir = root.join(AUTORUN_DIR);
    let script = run_dir.join(DRAIN_SCRIPT);
    if !script.exists() {
        fs::create_dir_all(&run_dir)
            .map_err(|e| Error::Seeds(format!("{}: {e}", run_dir.display())))?;
        fs::write(&script, DRAIN)
            .map_err(|e| Error::Seeds(format!("{}: {e}", script.display())))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
                .map_err(|e| Error::Seeds(format!("{}: {e}", script.display())))?;
        }
    }
    Ok(staged)
}
