// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (ToDO) getlogin userlogin

use clap::Command;
use std::io::{Write, stdout};
use uucore::entries::uid2usr;
use uucore::process::getuid;
use uucore::translate;
use uucore::{error::UResult, show_error};

/// The login name of the principal this process runs as.
///
/// PEIOS-DIVERGENCE(identity): not `getlogin()`. That reads a utmp record for
/// the controlling terminal, and Peios keeps no utmp — nothing writes one — so
/// it has nothing to return and this command printed only an error.
///
/// The token is the record of who a process is, and its user SID projects to
/// the uid. `logname` asks for the *login* identity rather than the current
/// effective one, which is exactly the primary token: `getuid()` reads the
/// primary credential and is unaffected by impersonation, where `geteuid()`
/// follows the effective token. The POSIX question maps onto the KACS one
/// without stretching either.
///
/// The name itself comes back from authd through NSS, canonically qualified.
fn get_userlogin() -> Option<String> {
    uid2usr(getuid()).ok()
}

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _ = uucore::clap_localization::handle_clap_result(uu_app(), args)?;

    if let Some(userlogin) = get_userlogin() {
        writeln!(stdout(), "{userlogin}")?;
    } else {
        show_error!("{}", translate!("logname-error-no-login-name"));
    }

    Ok(())
}

pub fn uu_app() -> Command {
    Command::new("logname")
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template("logname"))
        .override_usage(translate!("logname-usage"))
        .about(translate!("logname-about"))
        .infer_long_args(true)
}
