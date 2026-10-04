// passwd ~ (peiosutils) — change your own password.
//
// A PGSS Logon client for the credential-change conversation (PGSS §2.20). The
// conversation itself — opening /run/logon.sock with `CredentialChangeStart`,
// carrying the authority's rounds and reading how it ended — is
// libauthd-client's `credential` module, shared with the GUI apps. What is
// here is the terminal: rendering each round, and saying how it ended.
//
// That is the whole of it, and deliberately so. Linux's passwd is two programs
// in one binary — a PAM front end, and a setuid editor of /etc/shadow for
// locking, ageing and deleting — and neither half exists here. There is no
// shadow file to edit and no uid 0 to be; an administrator sets another
// principal's password with `lps password`, and account policy is the
// authority's.
//
// Two properties follow from the protocol rather than from this file:
//
// - **It can only change your own password.** `CredentialChangeStart` has no
//   field naming a principal; the authority takes it from this process's
//   token. There is nothing to pass a username into.
// - **It does not know what a password is.** It renders the prompts it is sent
//   and returns the answers. Asking for the current password, asking twice for
//   the new one and deciding what is acceptable are the source's, so none of
//   that is here.

use std::io;

use clap::{Arg, ArgAction, Command};
use libauthd::LOGON_SOCKET_PATH;
use libauthd::Secret;
use libauthd_client::credential::{Abandon, Collector, Credentials, MessageSeverity, Refusal, Round};
use libtty as tty;
use uucore::error::{UResult, USimpleError, UUsageError};

/// A positional argument, accepted only so that giving one gets an answer
/// rather than clap's "unexpected argument".
const NAME: &str = "name";

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

    Credentials::new()
        .change_password(&mut Terminal)
        .map_err(|refusal| USimpleError::new(1, explain(&refusal)))?;
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

/// What `passwd` says about a change that did not happen — in its own words,
/// about a password, where the shared conversation's are about any change.
fn explain(refusal: &Refusal) -> String {
    match refusal {
        Refusal::Unreachable(error) => match error.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => {
                format!("cannot reach the authority at {LOGON_SOCKET_PATH}; is authd running?")
            }
            io::ErrorKind::PermissionDenied => format!(
                "permission denied connecting to {LOGON_SOCKET_PATH}; this machine does not \
                 let you change your own password"
            ),
            _ => format!("cannot reach the authority at {LOGON_SOCKET_PATH}: {error}"),
        },
        Refusal::Unsent(what) | Refusal::Unrenderable(what) => what.clone(),
        // The terminal stops only when it cannot read, and says why.
        Refusal::Abandoned { reason, .. } => reason.clone(),
        Refusal::Unknown(what) => format!("{what}; whether the password changed is not known"),
        Refusal::Declined {
            code,
            denial,
            reason,
        } => {
            let reason = if !reason.is_empty() {
                reason.clone()
            } else if let Some(denial) = denial {
                format!("{denial:?}")
            } else {
                format!("refused with denial {code}")
            };
            format!("{reason} The password is unchanged.")
        }
    }
}

/// The real terminal — or, when standard input is not one, a pipe read a line
/// per prompt.
struct Terminal;

impl Collector for Terminal {
    fn round(&mut self, round: &Round) -> Result<Vec<Secret>, Abandon> {
        for message in &round.messages {
            match message.severity {
                MessageSeverity::Info => {
                    let _ = tty::show(&message.text);
                }
                MessageSeverity::Error => eprintln!("{}", message.text),
            }
        }
        round
            .prompts
            .iter()
            .map(|ask| {
                // Down a pipe there is nobody to show a prompt to, and a
                // fixed-size read could swallow the lines meant for the
                // prompts after this one.
                if tty::stdin_is_a_terminal() {
                    tty::prompt_secret(&format!("{}: ", ask.label))
                } else {
                    tty::read_line_secret()
                }
                .map_err(|error| Abandon::new(format!("could not read the password: {error}")))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    // The conversation is tested where it lives, in libauthd-client. What is
    // left here is what passwd says.
    use super::*;
    use libauthd::Denial;

    #[test]
    fn a_denial_says_the_password_is_unchanged() {
        let said = explain(&Refusal::Declined {
            code: 4,
            denial: Some(Denial::AuthenticationFailed),
            reason: "Authentication failed.".into(),
        });
        assert_eq!(said, "Authentication failed. The password is unchanged.");
    }

    #[test]
    fn a_denial_without_words_is_named() {
        let said = explain(&Refusal::Declined {
            code: 6,
            denial: Some(Denial::AccountRestricted),
            reason: String::new(),
        });
        assert_eq!(said, "AccountRestricted The password is unchanged.");
    }

    /// PGSS client obligation 4: an authority that goes away has not said how
    /// the change ended, and passwd must not claim to know.
    #[test]
    fn a_lost_authority_leaves_the_outcome_unknown() {
        let said = explain(&Refusal::Unknown("lost the authority: broken pipe".into()));
        assert_eq!(
            said,
            "lost the authority: broken pipe; whether the password changed is not known"
        );
    }

    #[test]
    fn an_absent_authority_asks_whether_authd_is_running() {
        let said = explain(&Refusal::Unreachable(io::ErrorKind::NotFound.into()));
        assert_eq!(
            said,
            "cannot reach the authority at /run/logon.sock; is authd running?"
        );
        let said = explain(&Refusal::Unreachable(io::ErrorKind::PermissionDenied.into()));
        assert!(said.starts_with("permission denied connecting to /run/logon.sock"));
    }

    #[test]
    fn a_terminal_that_cannot_read_says_so() {
        let said = explain(&Refusal::Abandoned {
            reason: "could not read the password: end of file".into(),
            last_error: None,
        });
        assert_eq!(said, "could not read the password: end of file");
    }
}
