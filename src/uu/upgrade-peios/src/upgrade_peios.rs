// upgrade-peios ~ (peiosutils) — entry point.
//
// A Peios release is a package: the edition package (peios-experimental,
// later peios-pro and friends) whose version is the OS version and whose
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

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

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
# once the queue is empty.
exec /bin/reg apply --dir /lcl/policy/autoapply.d --once-delete
";

pub type Result<T> = std::result::Result<T, Error>;

/// Exit codes: 1 usage, 2 no edition / release data, 3 peipkg failed,
/// 4 seed staging failed, 5 reg apply failed.
#[derive(Debug)]
pub enum Error {
    Usage(String),
    NoRelease(String),
    Peipkg(Option<i32>),
    Seeds(String),
    Apply(Option<i32>),
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) => 1,
            Error::NoRelease(_) => 2,
            Error::Peipkg(_) => 3,
            Error::Seeds(_) => 4,
            Error::Apply(_) => 5,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(m) | Error::NoRelease(m) | Error::Seeds(m) => f.write_str(m),
            Error::Peipkg(code) => write!(f, "peipkg upgrade failed{}", exit_suffix(*code)),
            Error::Apply(code) => write!(f, "reg apply failed{}", exit_suffix(*code)),
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
    let root = PathBuf::from(
        m.get_one::<String>("root")
            .map(String::as_str)
            .unwrap_or("/"),
    );
    let live = root == Path::new("/");
    let on_reboot = m.get_flag("on-reboot") || !live;
    let mut out = io::stdout().lock();

    let edition = edition(&root)?;

    if !m.get_flag("seeds-only") {
        writeln!(out, "upgrade-peios: upgrading {edition}").ok();
        let mut cmd = Command::new("peipkg");
        if !live {
            cmd.arg("--root").arg(&root);
        }
        cmd.arg("upgrade")
            .arg(&edition)
            .arg("--bypass-alternate-upgrade");
        if m.get_flag("yes") {
            cmd.arg("--yes");
        }
        let status = cmd
            .status()
            .map_err(|e| Error::Usage(format!("cannot run peipkg: {e}")))?;
        if !status.success() {
            return Err(Error::Peipkg(status.code()));
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

/// The edition package name, from os-release: `peios-<VARIANT_ID>`. The
/// edition package is what wrote os-release, so this is the package
/// database's own answer read from the file it owns.
fn edition(root: &Path) -> Result<String> {
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
        Some(v) if !v.is_empty() => Ok(format!("peios-{v}")),
        _ => Err(Error::NoRelease(format!(
            "{}: no VARIANT_ID; cannot tell which edition is installed",
            path.display()
        ))),
    }
}

fn unquote(v: &str) -> String {
    v.trim().trim_matches('"').to_string()
}

#[derive(serde::Deserialize)]
struct Release {
    #[serde(default)]
    registry: Registry,
}

#[derive(Default, serde::Deserialize)]
struct Registry {
    #[serde(default)]
    autoapply: Vec<String>,
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
