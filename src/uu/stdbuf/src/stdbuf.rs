// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (ToDO) tempdir dyld dylib optgrps libstdbuf
#[cfg(not(unix))]
compile_error!("stdbuf is not supported on the target");

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::ffi::OsString;
#[cfg(not(feature = "feat_external_libstdbuf"))]
use std::fs::Permissions;
#[cfg(not(feature = "feat_external_libstdbuf"))]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process;
#[cfg(not(feature = "feat_external_libstdbuf"))]
use tempfile::TempDir;
use thiserror::Error;
use uucore::display::Quotable;
use uucore::error::{UResult, USimpleError, UUsageError};
use uucore::format_usage;
use uucore::parser::parse_size::parse_size_u64;
use uucore::translate;

mod options {
    pub const INPUT: &str = "input";
    pub const INPUT_SHORT: char = 'i';
    pub const OUTPUT: &str = "output";
    pub const OUTPUT_SHORT: char = 'o';
    pub const ERROR: &str = "error";
    pub const ERROR_SHORT: char = 'e';
    pub const COMMAND: &str = "command";
}

#[cfg(not(feature = "feat_external_libstdbuf"))]
const STDBUF_INJECT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libstdbuf.so"));

enum BufferType {
    Default,
    Line,
    Size(usize),
}

struct ProgramOptions {
    stdin: BufferType,
    stdout: BufferType,
    stderr: BufferType,
}

impl TryFrom<&ArgMatches> for ProgramOptions {
    type Error = ProgramOptionsError;

    fn try_from(matches: &ArgMatches) -> Result<Self, Self::Error> {
        Ok(Self {
            stdin: check_option(matches, options::INPUT)?,
            stdout: check_option(matches, options::OUTPUT)?,
            stderr: check_option(matches, options::ERROR)?,
        })
    }
}

#[derive(Debug, Error)]
enum ProgramOptionsError {
    #[error("{}", translate!("stdbuf-error-line-buffering-stdin-meaningless"))]
    LineBufferingStdinMeaningless,
    #[error("{}", translate!("stdbuf-error-invalid-mode", "error" => _0.clone()))]
    InvalidMode(String),
    #[error("{}", translate!("stdbuf-error-value-too-large", "value" => _0.clone()))]
    ValueTooLarge(String),
}

fn preload_strings() -> (&'static str, &'static str) {
    ("LD_PRELOAD", "so")
}

fn check_option(matches: &ArgMatches, name: &str) -> Result<BufferType, ProgramOptionsError> {
    match matches.get_one::<String>(name) {
        Some(value) => match value.as_str() {
            "L" => {
                if name == options::INPUT {
                    Err(ProgramOptionsError::LineBufferingStdinMeaningless)
                } else {
                    Ok(BufferType::Line)
                }
            }
            x => parse_size_u64(x).map_or_else(
                |e| Err(ProgramOptionsError::InvalidMode(e.to_string())),
                |m| {
                    Ok(BufferType::Size(m.try_into().map_err(|_| {
                        ProgramOptionsError::ValueTooLarge(x.to_string())
                    })?))
                },
            ),
        },
        None => Ok(BufferType::Default),
    }
}

fn set_command_env(command: &mut process::Command, buffer_name: &str, buffer_type: &BufferType) {
    match buffer_type {
        BufferType::Size(m) => {
            command.env(buffer_name, m.to_string());
        }
        BufferType::Line => {
            command.env(buffer_name, "L");
        }
        BufferType::Default => {}
    }
}

#[cfg(not(feature = "feat_external_libstdbuf"))]
fn get_preload_env(tmp_dir: &TempDir) -> UResult<(String, PathBuf)> {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let (preload, extension) = preload_strings();
    let inject_path = tmp_dir.path().join("libstdbuf").with_extension(extension);

    // Create the library with an explicit 0600 so a permissive umask cannot
    // leave a world-writable .so behind for someone else to swap out before
    // the loader maps it.
    let mut open_options = OpenOptions::new();
    open_options.write(true).create_new(true).mode(0o600);
    let mut file = open_options.open(&inject_path)?;
    file.write_all(STDBUF_INJECT)?;

    Ok((preload.to_owned(), inject_path))
}

#[cfg(feature = "feat_external_libstdbuf")]
fn get_preload_env() -> UResult<(String, PathBuf)> {
    // Use the directory provided at compile time via LIBSTDBUF_DIR environment variable
    // This will fail to compile if LIBSTDBUF_DIR is not set, which is the desired behavior
    const LIBSTDBUF_DIR: &str = env!("LIBSTDBUF_DIR");

    let (preload, extension) = preload_strings();

    // Search paths in order:
    // 1. Directory where stdbuf is located (program_path)
    // 2. Compile-time directory from LIBSTDBUF_DIR
    let mut search_paths: Vec<PathBuf> = Vec::with_capacity(2);

    // First, try to get the directory where stdbuf is running from
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            search_paths.push(exe_dir.to_path_buf());
        }
    }

    // Add the compile-time directory as fallback
    search_paths.push(PathBuf::from(LIBSTDBUF_DIR));

    // Search for libstdbuf in each path
    for base_path in search_paths {
        let path_buf = base_path.join("libstdbuf").with_extension(extension);
        if path_buf.exists() {
            return Ok((preload.to_owned(), path_buf));
        }
    }

    // If not found in any path, report error
    let path_buf = PathBuf::from(LIBSTDBUF_DIR)
        .join("libstdbuf")
        .with_extension(extension);
    Err(USimpleError::new(
        1,
        translate!("stdbuf-error-external-libstdbuf-not-found", "path" => path_buf.display()),
    ))
}

/// The exit status to report for a child that has already terminated.
///
/// `exec()` would have let the shell observe the child's own fate directly.
/// Now that a waiter sits in between, reproduce it: a child killed by a signal
/// is reported as `128 + signal`, the convention shells use, instead of the 0
/// that `ExitStatus::code()` yields for a signalled process.
fn exit_status_code(status: process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    if let Some(signal) = status.signal() {
        return 128 + signal;
    }
    status.code().unwrap_or(1)
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches =
        uucore::clap_localization::handle_clap_result_with_exit_code(uu_app(), args, 125)?;

    let options =
        ProgramOptions::try_from(&matches).map_err(|e| UUsageError::new(125, e.to_string()))?;

    let mut command_values = matches
        .get_many::<OsString>(options::COMMAND)
        .ok_or_else(|| UUsageError::new(125, "no command specified"))?;
    let Some(first_command) = command_values.next() else {
        return Err(UUsageError::new(125, "no command specified"));
    };
    let mut command = process::Command::new(first_command);
    let command_params: Vec<&OsString> = command_values.collect();

    // When embedding the library, extract it into a temporary directory that is
    // private to this user. The mode is applied at mkdir time (not by a later
    // chmod) so the directory is never world-accessible in between. The TempDir
    // is kept alive until the child exits, so the loader can still find the
    // library, and is removed afterwards instead of being leaked.
    #[cfg(not(feature = "feat_external_libstdbuf"))]
    let (tmp_dir, preload_env, libstdbuf) = {
        let tmp_dir = tempfile::Builder::new()
            .permissions(Permissions::from_mode(0o700))
            .tempdir()
            .map_err(|e| UUsageError::new(125, format!("failed to create temp directory: {e}")))?;
        let (preload_env, libstdbuf) = get_preload_env(&tmp_dir)?;
        (tmp_dir, preload_env, libstdbuf)
    };
    #[cfg(feature = "feat_external_libstdbuf")]
    let (preload_env, libstdbuf) = get_preload_env()?;

    // The preload variable is a colon-separated list with no escaping mechanism,
    // so a path containing ':' does not round-trip: the dynamic loader splits it
    // and treats the leading component as a library to load. Since the temp
    // directory is derived from $TMPDIR, that component would be attacker-chosen
    // whenever TMPDIR crosses a privilege boundary. Refuse instead of preloading
    // something we did not select.
    if libstdbuf.as_os_str().as_encoded_bytes().contains(&b':') {
        return Err(USimpleError::new(
            125,
            translate!("stdbuf-error-preload-path-separator", "path" => libstdbuf.quote(), "var" => preload_env),
        ));
    }
    command.env(preload_env, libstdbuf);
    set_command_env(&mut command, "_STDBUF_I", &options.stdin);
    set_command_env(&mut command, "_STDBUF_O", &options.stdout);
    set_command_env(&mut command, "_STDBUF_E", &options.stderr);
    command.args(command_params);

    // Spawn a child and wait for it, rather than exec()ing, so this process
    // survives to drop the TempDir holding libstdbuf.so. exec() replaced the
    // process image before the destructor could run, leaking one temporary
    // directory per invocation.
    let e = match command.spawn() {
        Ok(mut child) => {
            let status = child.wait();
            // The child is gone: the library is no longer needed, remove it.
            #[cfg(not(feature = "feat_external_libstdbuf"))]
            drop(tmp_dir);
            let status = status.map_err(|e| {
                USimpleError::new(
                    1,
                    translate!("stdbuf-error-failed-to-execute", "error" => e),
                )
            })?;
            process::exit(exit_status_code(status));
        }
        Err(err) => err,
    };
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => Err(USimpleError::new(
            126,
            translate!("stdbuf-error-permission-denied"),
        )),
        std::io::ErrorKind::NotFound => Err(USimpleError::new(
            127,
            translate!("stdbuf-error-no-such-file"),
        )),
        _ => Err(USimpleError::new(
            1,
            translate!("stdbuf-error-failed-to-execute", "error" => e),
        )),
    }
}

pub fn uu_app() -> Command {
    Command::new("stdbuf")
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template("stdbuf"))
        .about(translate!("stdbuf-about"))
        .after_help(translate!("stdbuf-after-help"))
        .override_usage(format_usage(&translate!("stdbuf-usage")))
        .trailing_var_arg(true)
        .infer_long_args(true)
        .arg(
            Arg::new(options::INPUT)
                .long(options::INPUT)
                .short(options::INPUT_SHORT)
                .help(translate!("stdbuf-help-input"))
                .value_name("MODE")
                .required_unless_present_any([options::OUTPUT, options::ERROR]),
        )
        .arg(
            Arg::new(options::OUTPUT)
                .long(options::OUTPUT)
                .short(options::OUTPUT_SHORT)
                .help(translate!("stdbuf-help-output"))
                .value_name("MODE")
                .required_unless_present_any([options::INPUT, options::ERROR]),
        )
        .arg(
            Arg::new(options::ERROR)
                .long(options::ERROR)
                .short(options::ERROR_SHORT)
                .help(translate!("stdbuf-help-error"))
                .value_name("MODE")
                .required_unless_present_any([options::INPUT, options::OUTPUT]),
        )
        .arg(
            Arg::new(options::COMMAND)
                .action(ArgAction::Append)
                .hide(true)
                .required(true)
                .value_hint(clap::ValueHint::CommandName)
                .value_parser(clap::value_parser!(OsString)),
        )
}
