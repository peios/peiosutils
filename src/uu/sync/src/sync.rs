// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

/* Last synced with: sync (GNU coreutils) 8.13 */

use clap::{Arg, ArgAction, Command};
use nix::errno::Errno;
use nix::fcntl::{OFlag, open};
use nix::sys::stat::Mode;
use std::path::Path;
use uucore::display::Quotable;
use uucore::error::{UResult, USimpleError, get_exit_code, set_exit_code};
use uucore::format_usage;
use uucore::show_error;
use uucore::translate;

pub mod options {
    pub static FILE_SYSTEM: &str = "file-system";
    pub static DATA: &str = "data";
}

static ARG_FILES: &str = "files";

#[cfg(unix)]
mod platform {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use nix::fcntl::{FcntlArg, OFlag, fcntl};
    use nix::unistd::sync;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use nix::unistd::{fdatasync, syncfs};
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use std::fs::{File, OpenOptions};
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use std::os::unix::fs::OpenOptionsExt;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use uucore::display::Quotable;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use uucore::error::FromIo;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use uucore::translate;

    use uucore::error::UResult;

    #[expect(
        clippy::unnecessary_wraps,
        reason = "fn sig must match on all platforms"
    )]
    pub fn do_sync() -> UResult<()> {
        sync();
        Ok(())
    }

    /// Opens a file and resets its O_NONBLOCK flag to match GNU behavior.
    /// Returns the opened file or an error if opening fails.
    /// Logs a warning if fcntl fails but doesn't abort the operation.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn open_and_reset_nonblock(path: &str) -> UResult<File> {
        let f = OpenOptions::new()
            .read(true)
            .custom_flags(OFlag::O_NONBLOCK.bits())
            .open(path)
            .map_err_context(|| path.to_string())?;
        // Reset O_NONBLOCK flag if it was set (matches GNU behavior)
        // This is non-critical, so we log errors but don't fail
        if let Err(e) = fcntl(&f, FcntlArg::F_SETFL(OFlag::empty())) {
            use std::io::{Write, stderr};
            let _ = writeln!(
                stderr(),
                "sync: {}",
                translate!("sync-warning-fcntl-failed", "file" => path, "error" => e.to_string())
            );
            uucore::error::set_exit_code(1);
        }
        Ok(f)
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn do_sync_with<F>(files: Vec<String>, op: F) -> UResult<()>
    where
        F: Fn(File) -> Result<(), nix::Error>,
    {
        for path in files {
            let f = open_and_reset_nonblock(&path)?;
            op(f).map_err_context(
                || translate!("sync-error-syncing-file", "file" => path.quote()),
            )?;
        }
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn do_syncfs(files: Vec<String>) -> UResult<()> {
        do_sync_with(files, syncfs)
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn do_fdatasync(files: Vec<String>) -> UResult<()> {
        do_sync_with(files, fdatasync)
    }
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    let files: Vec<String> = matches
        .get_many::<String>(ARG_FILES)
        .map(|v| v.map(ToString::to_string).collect())
        .unwrap_or_default();

    if matches.get_flag(options::DATA) && files.is_empty() {
        return Err(USimpleError::new(
            1,
            translate!("sync-error-data-needs-argument"),
        ));
    }

    for f in &files {
        // Use the Nix open to be able to set the NONBLOCK flags for fifo files
        let path = Path::new(&f);
        if let Err(e) = open(path, OFlag::O_NONBLOCK, Mode::empty()) {
            if e != Errno::EACCES || (e == Errno::EACCES && path.is_dir()) {
                show_error!(
                    "{}",
                    translate!("sync-error-opening-file", "file" => f.quote(), "err" => e.desc())
                );
                set_exit_code(1);
            }
        }
    }

    if get_exit_code() != 0 {
        return Err(USimpleError::new(1, ""));
    }

    #[allow(clippy::if_same_then_else)]
    if matches.get_flag(options::FILE_SYSTEM) {
        if files.is_empty() {
            sync()?;
        } else {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            syncfs(files)?;
        }
    } else if matches.get_flag(options::DATA) {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        fdatasync(files)?;
    } else {
        sync()?;
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new("sync")
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template("sync"))
        .about(translate!("sync-about"))
        .override_usage(format_usage(&translate!("sync-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(options::FILE_SYSTEM)
                .short('f')
                .long(options::FILE_SYSTEM)
                .conflicts_with(options::DATA)
                .help(translate!("sync-help-file-system"))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::DATA)
                .short('d')
                .long(options::DATA)
                .conflicts_with(options::FILE_SYSTEM)
                .help(translate!("sync-help-data"))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(ARG_FILES)
                .action(ArgAction::Append)
                .value_hint(clap::ValueHint::AnyPath),
        )
}

fn sync() -> UResult<()> {
    platform::do_sync()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn syncfs(files: Vec<String>) -> UResult<()> {
    platform::do_syncfs(files)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn fdatasync(files: Vec<String>) -> UResult<()> {
    platform::do_fdatasync(files)
}
