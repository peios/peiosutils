// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (ToDO) getlogin userlogin

use clap::Command;
use std::ffi::CStr;
use std::io::{Write, stdout};
use uucore::translate;
use uucore::{
    error::{UResult, USimpleError},
    show_error,
};

fn get_userlogin() -> Option<String> {
    let login_ptr = unsafe { libc::getlogin() };
    if login_ptr.is_null() {
        None
    } else {
        Some(String::from_utf8_lossy(unsafe { CStr::from_ptr(login_ptr) }.to_bytes()).to_string())
    }
}

#[uucore::main(no_signals)]
#[allow(
    unreachable_code,
    unused_variables,
    reason = "logname is a deliberate not-implemented stub; the real \
              implementation below is preserved, unreachable, pending authd"
)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    // logname is intentionally a no-op stub on Peios for now.
    //
    // It prints the login name recorded for the controlling terminal
    // (libc::getlogin(), backed by utmp). "Login name" is an authd
    // identity/session concept that does not exist yet — there is no
    // login-session model to query. Stubbed like the groups and id
    // commands; the real implementation is preserved below, unreachable,
    // and will be wired up once authd lands.
    return Err(USimpleError::new(
        1,
        "not implemented on Peios yet (pending the authd identity model)".to_string(),
    ));

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
