// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! `mkfifo` for Peios. Creates FIFOs (named pipes).
//!
//! POSIX mode bits, umask, and SELinux contexts do not exist on Peios.
//! A FIFO is created with the Linux `mkfifo()` namespace syscall — KACS
//! computes its Security Descriptor from the parent directory's
//! inheritable ACEs. The `--sd*` flag group (see `uucore::sd_control`)
//! overrides that, applied post-create with `libp_sd::set_sd`.

use clap::{Arg, ArgAction, Command};
use nix::sys::stat::Mode;
use nix::unistd::mkfifo;
use std::fs;
use std::path::Path;
use uucore::display::Quotable;
use uucore::error::{UResult, USimpleError};
use uucore::sd_control::{self, CreatorSd};
use uucore::translate;
use uucore::{format_usage, show_if_err};

mod options {
    pub static FIFO: &str = "fifo";
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;

    let creator_sd = sd_control::creator_sd_from_matches(&matches)?;

    let fifos: Vec<String> = match matches.get_many::<String>(options::FIFO) {
        Some(v) => v.cloned().collect(),
        None => {
            return Err(USimpleError::new(
                1,
                translate!("mkfifo-error-missing-operand"),
            ));
        }
    };

    for f in fifos {
        show_if_err!(make_fifo(&f, creator_sd.as_ref()));
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new("mkfifo")
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template("mkfifo"))
        .override_usage(format_usage(&translate!("mkfifo-usage")))
        .about(translate!("mkfifo-about"))
        .infer_long_args(true)
        .args(sd_control::args())
        .arg(
            Arg::new(options::FIFO)
                .hide(true)
                .action(ArgAction::Append)
                .value_hint(clap::ValueHint::AnyPath),
        )
}

/// Create one FIFO and, when `sd` is given, apply it post-create.
///
/// The FIFO is created with the Linux `mkfifo()` namespace syscall —
/// KACS-native open does not create special nodes — so the descriptor
/// is applied afterward with `libp_sd::set_sd` (create-then-set; see
/// `peios/sd-creation-design.md`). If the descriptor cannot be applied
/// the FIFO is removed, leaving nothing half-secured behind.
fn make_fifo(path: &str, sd: Option<&CreatorSd>) -> UResult<()> {
    // The raw Linux inode mode is compatibility metadata only on Peios;
    // KACS computes the FIFO's real Security Descriptor.
    mkfifo(path, Mode::from_bits_truncate(0o666)).map_err(|_| {
        USimpleError::new(
            1,
            translate!("mkfifo-error-cannot-create-fifo", "path" => path.quote()),
        )
    })?;

    if let Some(sd) = sd
        && let Err(e) = sd.apply_to(Path::new(path))
    {
        let _ = fs::remove_file(path);
        return Err(e);
    }

    Ok(())
}
