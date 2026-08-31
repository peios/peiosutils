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
    error::{UError, UResult},
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
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    // PEIOS-DIVERGENCE(identity): the two query paths reach the answer by
    // different routes, but the same constraint truncates both, so today they
    // agree -- and both are much shorter than the token.
    //
    // With no username, the gids come from the process credential: the gids
    // the kernel projected from the token's group set. With a username, they
    // come from the passwd/group path, which reaches authd's Lookup and
    // returns recorded memberships.
    //
    // A group survives either route only if its SID has a projected gid, and
    // most do not. Measured on a live image (PEI-206), an administrator's
    // token holds seven groups and both forms print two -- the primary group
    // and the recorded membership. Everyone (S-1-1-0), Local (S-1-2-0),
    // Interactive (S-1-5-4), the logon session and the user's own SID all
    // carry no gid and appear in neither.
    //
    // Everyone's absence is the one that misleads: it grants real access
    // (an ACE allowing Everyone grants the caller) through a group this
    // command reports the caller is not in. Interactive/Network/Batch/Service
    // are unnumbered deliberately, so logon-type policy is expressible.
    //
    // SE_GROUP_* attributes also have nowhere to go, so a deny-only group is
    // indistinguishable from a granting one. `token show --all` is the
    // lossless view.
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
