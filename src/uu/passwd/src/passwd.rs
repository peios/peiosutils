// passwd ~ (peiosutils) changes the calling account's password through authd.
// The shared library owns the protocol; this applet collects terminal input.

use std::io;

use auth_core::{Denial, LOGON_SOCKET_PATH, Secret};
use clap::{Arg, ArgAction, Command};
use peios_auth::{Answer, Capabilities, Client, Error, Progress, Round};
use terminal_input as tty;
use uucore::error::{UResult, USimpleError, UUsageError};

const NAME: &str = "name";
// Stable public constants from peios/auth.h.
const ERROR_UNAVAILABLE: u32 = 1;
const ERROR_PGSS_DENIAL: u32 = 6;
const MESSAGE_ERROR: u32 = 1;
const CREDENTIAL_PASSWORD: u32 = 1;

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match uu_app().try_get_matches_from(args) {
        Ok(matches) => matches,
        Err(error) => {
            let code = error.exit_code();
            error.print().ok();
            return if code == 0 {
                Ok(())
            } else {
                Err(USimpleError::new(code, ""))
            };
        }
    };
    if matches.contains_id(NAME) {
        return Err(UUsageError::new(
            2,
            "this changes only your own password; an administrator sets another \
             principal's with 'lps password'",
        ));
    }
    let client = Client::new().map_err(|error| USimpleError::new(1, explain(&error)))?;
    change_password(&client, read_round).map_err(|message| USimpleError::new(1, message))?;
    println!("password changed");
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .about("Change your own password")
        .override_usage("passwd")
        .after_help(
            "Asks for your current password and then a new one, as the authority \
             directs. When standard input is not a terminal, one line is read for each \
             prompt, in order, and nothing is printed for them.",
        )
        .arg(
            Arg::new(NAME)
                .hide(true)
                .num_args(1..)
                .action(ArgAction::Append),
        )
}

/// Complete one credential conversation without retrying an uncertain mutation.
fn change_password(
    client: &Client,
    mut collect: impl FnMut(&Round) -> Result<Vec<Secret>, String>,
) -> Result<(), String> {
    let mut operation = client
        .change_password(Capabilities::PASSWORD)
        .map_err(|e| explain(&e))?;
    loop {
        match operation.wait().map_err(|error| explain(&error))? {
            Progress::Complete => return Ok(()),
            Progress::Wait => continue,
            Progress::Prompt => {}
        }
        let round = operation.round().map_err(|error| explain(&error))?;
        let secrets = match collect(&round) {
            Ok(secrets) => secrets,
            Err(message) => {
                operation.cancel();
                return Err(message);
            }
        };
        if secrets.len() != round.prompts.len() {
            operation.cancel();
            return Err("could not collect every requested credential".into());
        }
        let answers: Vec<_> = round
            .prompts
            .iter()
            .zip(&secrets)
            .map(|(prompt, secret)| Answer {
                credential_ref: prompt.credential_ref,
                data: secret.expose(),
            })
            .collect();
        operation
            .respond(round.id, &answers)
            .map_err(|error| explain(&error))?;
        // Both the caller's Secret values and the library's encoded response are wiped.
        // No credential material survives here into the next prompt round.
    }
}

/// Distinguish an explicit refusal from a request whose outcome is unknown.
fn explain(error: &Error) -> String {
    if error.outcome_unknown {
        return format!(
            "{}; whether the password changed is not known",
            error.message
        );
    }
    if error.category == ERROR_PGSS_DENIAL {
        let reason = if !error.message.is_empty() {
            error.message.clone()
        } else if let Some(denial) = Denial::from_u32(error.code) {
            format!("{denial:?}")
        } else {
            format!("refused with denial {}", error.code)
        };
        return format!("{reason} The password is unchanged.");
    }
    if error.category == ERROR_UNAVAILABLE {
        let kind = i32::try_from(error.code)
            .ok()
            .map(|code| io::Error::from_raw_os_error(code).kind());
        return match kind {
            Some(io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {
                format!("cannot reach the authority at {LOGON_SOCKET_PATH}; is authd running?")
            }
            Some(io::ErrorKind::PermissionDenied) => format!(
                "permission denied connecting to {LOGON_SOCKET_PATH}; this machine does not \
                 let you change your own password"
            ),
            _ => format!(
                "cannot reach the authority at {LOGON_SOCKET_PATH}: {}",
                error.message
            ),
        };
    }
    error.message.clone()
}

/// Read one line per password prompt, retaining the shared terminal cleanup behavior.
fn read_round(round: &Round) -> Result<Vec<Secret>, String> {
    for message in &round.messages {
        if message.severity == MESSAGE_ERROR {
            eprintln!("{}", message.text);
        } else {
            let _ = tty::show(&message.text);
        }
    }
    round
        .prompts
        .iter()
        .map(|prompt| {
            if prompt.credential_type != CREDENTIAL_PASSWORD {
                return Err("the authority requested an unsupported credential".into());
            }
            if tty::stdin_is_a_terminal() {
                tty::prompt_secret(&format!("{}: ", prompt.label))
            } else {
                tty::read_line_secret()
            }
            .map_err(|error| format!("could not read the password: {error}"))
        })
        .collect()
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
