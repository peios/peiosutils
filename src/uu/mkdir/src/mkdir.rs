// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! `mkdir` for Peios. Creates directories.
//!
//! POSIX mode bits, umask, and SELinux contexts do not exist on Peios —
//! a new directory's Security Descriptor is computed by KACS from the
//! parent's inheritable ACEs. The `--sd*` flag group (see
//! `uucore::sd_control`) lets the caller override that; with no SD flag a
//! directory simply takes plain kernel inheritance.

use clap::builder::ValueParser;
use clap::{Arg, ArgAction, Command};
use std::ffi::OsString;
use std::fs;
use std::io::{Write, stdout};
use std::path::Path;
use uucore::error::{UResult, USimpleError};
use uucore::sd_control::{self, CreatorSd};
use uucore::translate;
use uucore::{display::Quotable, fs::dir_strip_dot_for_creation};
use uucore::{format_usage, show_if_err};

mod options {
    pub const PARENTS: &str = "parents";
    pub const VERBOSE: &str = "verbose";
    pub const DIRS: &str = "dirs";
}

/// Configuration for a `mkdir` run.
struct Config {
    /// Create parent directories as needed (`-p`).
    recursive: bool,
    /// Print a message for each created directory (`-v`).
    verbose: bool,
    /// Security descriptor to apply to the final target, from the
    /// `--sd*` flag group. `None` means plain kernel inheritance.
    creator_sd: Option<CreatorSd>,
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;

    let config = Config {
        recursive: matches.get_flag(options::PARENTS),
        verbose: matches.get_flag(options::VERBOSE),
        creator_sd: sd_control::creator_sd_from_matches(&matches)?,
    };

    let dirs = matches
        .get_many::<OsString>(options::DIRS)
        .unwrap_or_default();
    for dir in dirs {
        show_if_err!(mkdir(Path::new(dir), &config));
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new("mkdir")
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template("mkdir"))
        .about(translate!("mkdir-about"))
        .override_usage(format_usage(&translate!("mkdir-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(options::PARENTS)
                .short('p')
                .long(options::PARENTS)
                .help(translate!("mkdir-help-parents"))
                .overrides_with(options::PARENTS)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::VERBOSE)
                .short('v')
                .long(options::VERBOSE)
                .help(translate!("mkdir-help-verbose"))
                .action(ArgAction::SetTrue),
        )
        .args(sd_control::args())
        .arg(
            Arg::new(options::DIRS)
                .action(ArgAction::Append)
                .num_args(1..)
                .required(true)
                .value_parser(ValueParser::os_string())
                .value_hint(clap::ValueHint::DirPath),
        )
}

/// Create a directory at `path`, honoring `-p`.
///
/// To match GNU behavior, a path whose last component is a single dot
/// (`some/path/.`) is created with the dot stripped.
fn mkdir(path: &Path, config: &Config) -> UResult<()> {
    if path.as_os_str().is_empty() {
        return Err(USimpleError::new(
            1,
            translate!("mkdir-error-empty-directory-name"),
        ));
    }
    let path = dir_strip_dot_for_creation(path);
    create_dir(path.as_path(), config)
}

/// Create `path`, first creating missing ancestors when `-p` is set.
///
/// Ancestor handling is iterative — no recursion — so a very deep tree
/// cannot overflow the stack.
fn create_dir(path: &Path, config: &Config) -> UResult<()> {
    if path == Path::new("") {
        return Ok(());
    }

    if path.exists() {
        if config.recursive {
            // `-p`: an existing target is success, and keeps its current
            // SD — the `--sd*` flags apply only to a created object.
            return Ok(());
        }
        return Err(USimpleError::new(
            1,
            translate!("mkdir-error-file-exists", "path" => path.maybe_quote()),
        ));
    }

    if config.recursive {
        // Collect ancestors leaf-ward, then create them root-ward. Each
        // ancestor gets plain kernel inheritance — the `--sd*` flags
        // apply to the final target only.
        let mut ancestors: Vec<&Path> = Vec::new();
        let mut current = path;
        while let Some(parent) = current.parent() {
            if parent == Path::new("") {
                break;
            }
            ancestors.push(parent);
            current = parent;
        }
        for ancestor in ancestors.iter().rev() {
            if !ancestor.exists() {
                create_one(ancestor, config, None)?;
            }
        }
    }

    create_one(path, config, config.creator_sd.as_ref())
}

/// Create exactly one directory and, when `sd` is given, apply it.
///
/// The directory is created with the ordinary `mkdir()` namespace
/// syscall — KACS computes its inherited SD — and the descriptor, if
/// any, is applied afterward (create-then-set; see
/// `peios/sd-creation-design.md`). If the descriptor cannot be applied
/// the just-created directory is removed so no half-secured directory is
/// left behind.
fn create_one(path: &Path, config: &Config, sd: Option<&CreatorSd>) -> UResult<()> {
    match fs::create_dir(path) {
        Ok(()) => {}
        // Under `-p`, losing a creation race (or a `..`-laden path that
        // resolves to an existing directory) is success, not an error.
        Err(_) if config.recursive && path.is_dir() => return Ok(()),
        Err(e) => return Err(e.into()),
    }

    if config.verbose {
        writeln!(
            stdout(),
            "{}",
            translate!("mkdir-verbose-created-directory", "util_name" => "mkdir", "path" => path.quote())
        )?;
    }

    if let Some(sd) = sd
        && let Err(e) = sd.apply_to(path)
    {
        let _ = fs::remove_dir(path);
        return Err(e);
    }

    Ok(())
}
