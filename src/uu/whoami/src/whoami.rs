// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#![allow(unused)]

use clap::Command;
use std::ffi::OsString;
use uucore::display::println_verbatim;
use uucore::error::{FromIo, UResult};
use uucore::translate;

mod platform;

#[uucore::main(no_signals)]
pub fn uumain(_args: impl uucore::Args) -> UResult<()> {
    // whoami resolves the effective user ID to a passwd username. Peios
    // has no native identity model yet -- authd and the token/SID model
    // are undecided -- so the resolution is deferred. The whoami() helper
    // below is kept compiled for when that model lands.
    Err(uucore::error::deferred_on_peios(
        "a native Peios identity model (effective-user name resolution)",
    ))
}

/// Get the current username
pub fn whoami() -> UResult<OsString> {
    platform::get_username().map_err_context(|| translate!("whoami-error-failed-to-get"))
}

pub fn uu_app() -> Command {
    Command::new("whoami")
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template("whoami"))
        .about(translate!("whoami-about"))
        .override_usage(translate!("whoami-usage"))
        .infer_long_args(true)
}
