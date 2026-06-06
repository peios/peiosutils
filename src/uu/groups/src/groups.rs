// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (ToDO) passwd

use std::io::{Write, stdout};
use thiserror::Error;
use uucore::{
    display::Quotable,
    entries::{Locate, Passwd, get_groups_gnu, gid2grp},
    error::{UError, UResult, USimpleError},
    format_usage, show,
};

use clap::{Arg, ArgAction, Command};
use uucore::translate;

mod options {
    pub const USERS: &str = "USERNAME";
}

#[derive(Debug, Error)]
enum GroupsError {
    #[error("{message}", message = translate!("groups-error-fetch"))]
    GetGroupsFailed,

    #[error("{message} {gid}", message = translate!("groups-error-notfound"), gid = .0)]
    GroupNotFound(u32),

    #[error("{user}: {message}", user = .0.quote(), message = translate!("groups-error-user"))]
    UserNotFound(String),
}

impl UError for GroupsError {}

fn infallible_gid2grp(gid: u32) -> String {
    if let Ok(grp) = gid2grp(gid) {
        grp
    } else {
        // The `show!()` macro sets the global exit code for the program.
        show!(GroupsError::GroupNotFound(gid));
        gid.to_string()
    }
}

#[uucore::main(no_signals)]
#[allow(
    unreachable_code,
    unused_variables,
    reason = "groups is a deliberate not-implemented stub; the real \
              implementation below is preserved, unreachable, pending authd"
)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    // groups is intentionally a no-op stub on Peios for now.
    //
    // Its two query paths -- the process-credential path
    // (getgroups()/getegid(), used when no username is given) and the
    // /etc/passwd + /etc/group database lookup (used with a username) --
    // both depend on the Peios identity model, which is not settled
    // until authd exists. The original implementation is preserved
    // below, unreachable, and will be restored and adapted to projected
    // token identity once authd lands.
    return Err(USimpleError::new(
        1,
        "not implemented on Peios yet (pending the authd identity model)".to_string(),
    ));

    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;

    let users: Vec<String> = matches
        .get_many::<String>(options::USERS)
        .map(|v| v.map(ToString::to_string).collect())
        .unwrap_or_default();

    if users.is_empty() {
        let Ok(gids) = get_groups_gnu(None) else {
            return Err(GroupsError::GetGroupsFailed.into());
        };
        let groups: Vec<String> = gids.into_iter().map(infallible_gid2grp).collect();
        writeln!(stdout(), "{}", groups.join(" "))?;
        return Ok(());
    }

    for user in users {
        match Passwd::locate(user.as_str()) {
            Ok(p) => {
                let groups: Vec<String> =
                    p.belongs_to().into_iter().map(infallible_gid2grp).collect();
                writeln!(stdout(), "{user} : {}", groups.join(" "))?;
            }
            Err(_) => {
                // The `show!()` macro sets the global exit code for the program.
                show!(GroupsError::UserNotFound(user));
            }
        }
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new("groups")
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template("groups"))
        .about(translate!("groups-about"))
        .override_usage(format_usage(&translate!("groups-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(options::USERS)
                .action(ArgAction::Append)
                .value_name(options::USERS)
                .value_hint(clap::ValueHint::Username),
        )
}
