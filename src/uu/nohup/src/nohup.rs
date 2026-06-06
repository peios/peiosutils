// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (ToDO) SIGHUP cproc vprocmgr homeout
#[cfg(not(unix))]
compile_error!("nohup is not supported on the target");

use clap::{Arg, ArgAction, Command};
use rustix::stdio::{dup2_stderr, dup2_stdin, dup2_stdout, stdout};
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, IsTerminal};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::LazyLock;
use thiserror::Error;
use uucore::display::Quotable;
use uucore::error::{UError, UResult, set_exit_code};
use uucore::sd_control::{CreatorSd, OPT_SDDL, creator_sd_from_sddl};
use uucore::translate;
use uucore::{format_usage, show_error};

static NOHUP_OUT: &str = "nohup.out";
// The descriptor nohup writes onto a nohup.out it creates when --sddl is
// not given: a DACL-protected file whose sole ACE grants full access to
// the Owner Rights SID (S-1-3-4) — the security-descriptor form of GNU
// nohup's mode 0600. A pre-existing nohup.out keeps its own descriptor.
const NOHUP_OUT_SDDL: &str = "D:P(A;;FA;;;S-1-3-4)";
// exit codes that match the GNU implementation
static EXIT_CANCELED: i32 = 125;
static EXIT_CANNOT_INVOKE: i32 = 126;
static EXIT_ENOENT: i32 = 127;
static POSIX_NOHUP_FAILURE: i32 = 127;

mod options {
    pub const CMD: &str = "cmd";
}

#[derive(Debug, Error)]
enum NohupError {
    #[cfg(target_vendor = "apple")]
    #[error("{}", translate!("nohup-error-cannot-detach"))]
    CannotDetach,

    #[error("{}", translate!("nohup-error-cannot-replace", "name" => (*_0), "err" => _1))]
    CannotReplace(&'static str, #[source] Error),

    #[error("{}", translate!("nohup-error-open-failed", "path" => NOHUP_OUT.quote(), "err" => _1))]
    OpenFailed(i32, #[source] Error),

    #[error("{}", translate!("nohup-error-open-failed-both", "first_path" => NOHUP_OUT.quote(), "first_err" => _1, "second_path" => _2.quote(), "second_err" => _3))]
    OpenFailed2(i32, #[source] Error, String, Error),
}

impl UError for NohupError {
    fn code(&self) -> i32 {
        #[allow(clippy::match_wildcard_for_single_variants)]
        match self {
            Self::OpenFailed(code, _) | Self::OpenFailed2(code, _, _, _) => *code,
            _ => 2,
        }
    }
}

static FAILURE_CODE: LazyLock<i32> = LazyLock::new(|| {
    if env::var_os("POSIXLY_CORRECT").is_some() {
        POSIX_NOHUP_FAILURE
    } else {
        EXIT_CANCELED
    }
});

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result_with_exit_code(
        uu_app(),
        args,
        *FAILURE_CODE,
    )?;

    // Resolved up front so a malformed --sddl fails before nohup does any
    // work, even when stdout is redirected and no nohup.out is created.
    let creator_sd = match matches.get_one::<String>(OPT_SDDL) {
        Some(sddl) => creator_sd_from_sddl(sddl)?,
        None => creator_sd_from_sddl(NOHUP_OUT_SDDL)?,
    };

    replace_fds(&creator_sd)?;

    unsafe { libc::signal(libc::SIGHUP, libc::SIG_IGN) };

    #[cfg(target_vendor = "apple")]
    if unsafe { !_vprocmgr_detach_from_console(0).is_null() } {
        return Err(NohupError::CannotDetach.into());
    }
    #[allow(clippy::unwrap_used, reason = "set as required by clap")]
    let mut cmd_iter = matches.get_many::<String>(options::CMD).unwrap();
    #[allow(clippy::unwrap_used, reason = "set as required by clap")]
    let cmd = cmd_iter.next().unwrap();
    let args: Vec<&String> = cmd_iter.collect();

    let err = process::Command::new(cmd).args(args).exec();

    match err.kind() {
        ErrorKind::NotFound => set_exit_code(EXIT_ENOENT),
        _ => set_exit_code(EXIT_CANNOT_INVOKE),
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new("nohup")
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template("nohup"))
        .about(translate!("nohup-about"))
        .after_help(translate!("nohup-after-help"))
        .override_usage(format_usage(&translate!("nohup-usage")))
        .arg(
            Arg::new(OPT_SDDL)
                .long("sddl")
                .value_name("SDDL")
                .help(
                    "security descriptor (SDDL) for a nohup.out that nohup creates; \
                     the default grants the owner only",
                ),
        )
        .arg(
            Arg::new(options::CMD)
                .hide(true)
                .required(true)
                .action(ArgAction::Append)
                .value_hint(clap::ValueHint::CommandName),
        )
        .trailing_var_arg(true)
        .infer_long_args(true)
}

fn replace_fds(creator_sd: &CreatorSd) -> UResult<()> {
    if std::io::stdin().is_terminal() {
        let new_stdin = File::open(Path::new("/dev/null"))
            .map_err(|e| NohupError::CannotReplace("STDIN", e))?;
        dup2_stdin(&new_stdin).map_err(|e| NohupError::CannotReplace("STDIN", Error::from(e)))?;
    }

    if std::io::stdout().is_terminal() {
        let new_stdout = find_stdout(creator_sd)?;

        dup2_stdout(&new_stdout)
            .map_err(|e| NohupError::CannotReplace("STDOUT", Error::from(e)))?;
    }

    if std::io::stderr().is_terminal() {
        dup2_stderr(stdout()).map_err(|e| NohupError::CannotReplace("STDERR", Error::from(e)))?;
    }
    Ok(())
}

fn find_stdout(creator_sd: &CreatorSd) -> UResult<File> {
    try_open_nohup_file(NOHUP_OUT, creator_sd).or_else(|e1| {
        let Ok(home) = env::var("HOME") else {
            return Err(NohupError::OpenFailed(*FAILURE_CODE, e1).into());
        };

        let home_out = PathBuf::from(home).join(NOHUP_OUT);
        let home_out = home_out.to_str().unwrap();

        try_open_nohup_file(home_out, creator_sd).map_err(|e2| {
            NohupError::OpenFailed2(*FAILURE_CODE, e1, home_out.to_string(), e2).into()
        })
    })
}

fn try_open_nohup_file(path: &str, creator_sd: &CreatorSd) -> std::io::Result<File> {
    // create_new distinguishes a nohup.out that *nohup* made from a
    // pre-existing one: the creator descriptor is only ever applied to a
    // file nohup itself created.
    let file = match OpenOptions::new().append(true).create_new(true).open(path) {
        Ok(file) => {
            // Lock the fresh nohup.out down before any command output can
            // reach it. If that fails, unlink it — an under-secured
            // output file holding background-job output is worse than
            // none at all.
            if let Err(e) = creator_sd.apply_to(Path::new(path)) {
                let _ = std::fs::remove_file(path);
                return Err(Error::other(format!(
                    "could not secure {}: {e}",
                    path.quote()
                )));
            }
            file
        }
        // A pre-existing nohup.out is appended to with its descriptor
        // left intact — nohup never re-secures a file it did not create.
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            OpenOptions::new().append(true).open(path)?
        }
        Err(e) => return Err(e),
    };

    show_error!(
        "{}",
        translate!("nohup-ignoring-input-appending-output", "path" => path.quote())
    );

    Ok(file)
}

#[cfg(target_vendor = "apple")]
unsafe extern "C" {
    fn _vprocmgr_detach_from_console(flags: u32) -> *const libc::c_int;
}
