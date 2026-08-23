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
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    // PEIOS-DIVERGENCE(identity): the effective uid this resolves is a
    // projection of the token's user SID, and the name comes back from authd
    // through NSS. The name is the principal's; the number is not the
    // authority. `id -Z` prints the SID access is decided against.
    println_verbatim(whoami()?).map_err_context(|| translate!("whoami-error-failed-to-print"))
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
