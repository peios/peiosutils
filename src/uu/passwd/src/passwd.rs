// passwd ~ (peiosutils) — change your own password.
//
// A PGSS Logon client for the credential-change conversation (PGSS §2.20). It
// opens /run/logon.sock with `CredentialChangeStart`, renders whatever the
// authority asks for, and reports how the conversation ended.
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
use std::os::unix::net::UnixStream;

use clap::{Arg, ArgAction, Command};
use libauthd::LOGON_SOCKET_PATH;
use libauthd::Secret;
use libauthd::transport::{recv_message_with_fd, send_message};
use libauthd::wire::{
    self, Answer, CredentialChangeStart, CredentialResponse, CredentialType, MSG_ACCESS_DENIED,
    MSG_CREDENTIAL_CHANGED, MSG_CREDENTIAL_REQUEST, Message, MessageSeverity, Prompt,
    decode_access_denied, decode_credential_changed, decode_credential_request, decode_header,
    encode_credential_change_start, encode_credential_response,
};
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

    let socket = connect().map_err(|reason| USimpleError::new(1, reason))?;
    change(&socket, &mut Terminal).map_err(|reason| USimpleError::new(1, reason))?;
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

/// Open the logon socket, saying what went wrong in terms of the authority.
fn connect() -> Result<UnixStream, String> {
    UnixStream::connect(LOGON_SOCKET_PATH).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => {
            format!("cannot reach the authority at {LOGON_SOCKET_PATH}; is authd running?")
        }
        io::ErrorKind::PermissionDenied => format!(
            "permission denied connecting to {LOGON_SOCKET_PATH}; this machine does not \
             let you change your own password"
        ),
        _ => format!("cannot reach the authority at {LOGON_SOCKET_PATH}: {error}"),
    })
}

/// Where prompts are answered and messages shown.
///
/// A seam rather than calls into `libtty`, so the conversation can be driven
/// by a test without a terminal.
pub trait Collector {
    /// Collect the answer to one `Password` prompt.
    fn password(&mut self, prompt: &Prompt) -> io::Result<Secret>;
    /// Show one message from the authority.
    fn show(&mut self, message: &Message);
}

/// The real terminal — or, when standard input is not one, a pipe read a line
/// per prompt.
struct Terminal;

impl Collector for Terminal {
    fn password(&mut self, prompt: &Prompt) -> io::Result<Secret> {
        // Down a pipe there is nobody to show a prompt to, and a fixed-size
        // read could swallow the lines meant for the prompts after this one.
        if tty::stdin_is_a_terminal() {
            tty::prompt_secret(&format!("{}: ", prompt.credential_name))
        } else {
            tty::read_line_secret()
        }
    }

    fn show(&mut self, message: &Message) {
        match message.severity {
            MessageSeverity::Info => {
                let _ = tty::show(&message.text);
            }
            MessageSeverity::Error => eprintln!("{}", message.text),
        }
    }
}

/// Run one credential-change conversation to its terminal message.
///
/// `Ok` only on `CredentialChanged`. Every `Err` says whether the password
/// changed where that is known — and, where the authority went away before
/// saying, that it is not.
pub fn change(socket: &UnixStream, collector: &mut dyn Collector) -> Result<(), String> {
    let start = encode_credential_change_start(&CredentialChangeStart {
        // The only credential type there is, and the only one this renders.
        supported_credential_types: vec![CredentialType::Password],
    })
    .map_err(|error| format!("could not encode the request: {error:?}"))?;
    send_message(socket, &start)
        .map_err(|error| format!("could not reach the authority: {error}"))?;

    loop {
        // With a descriptor, in case one arrives: §2.20 expects none, and one
        // that does is closed here by being dropped rather than left behind.
        let (message, _descriptor) = recv_message_with_fd(&wire::FRAMING, socket)
            .map_err(|error| unknown_outcome(&format!("lost the authority: {error}")))?;

        let (message_type, _) = decode_header(message.expose())
            .map_err(|error| unknown_outcome(&format!("malformed reply: {error:?}")))?;

        match message_type {
            MSG_CREDENTIAL_REQUEST => {
                let request = decode_credential_request(message.expose()).map_err(|error| {
                    format!("the authority sent a request this cannot render: {error:?}")
                })?;
                for note in &request.messages {
                    collector.show(note);
                }

                let mut answers = Vec::with_capacity(request.prompts.len());
                for prompt in &request.prompts {
                    let data = match prompt.credential_type {
                        CredentialType::Password => collector
                            .password(prompt)
                            .map_err(|error| format!("could not read the password: {error}"))?,
                    };
                    answers.push(Answer {
                        credential_ref: prompt.credential_ref,
                        data,
                    });
                }

                let encoded = encode_credential_response(&CredentialResponse { answers })
                    .map_err(|error| format!("could not encode the answers: {error:?}"))?;
                send_message(socket, encoded.expose())
                    .map_err(|error| unknown_outcome(&format!("lost the authority: {error}")))?;
            }

            MSG_CREDENTIAL_CHANGED => {
                decode_credential_changed(message.expose())
                    .map_err(|error| unknown_outcome(&format!("malformed reply: {error:?}")))?;
                return Ok(());
            }

            MSG_ACCESS_DENIED => {
                let denied = decode_access_denied(message.expose())
                    .map_err(|error| unknown_outcome(&format!("malformed denial: {error:?}")))?;
                let reason = if denied.reason.is_empty() {
                    format!("{:?}", denied.denial)
                } else {
                    denied.reason
                };
                return Err(format!("{reason} The password is unchanged."));
            }

            other => {
                return Err(unknown_outcome(&format!(
                    "unexpected message {other:#06x} from the authority"
                )));
            }
        }
    }
}

/// A failure after the change was asked for and before the authority said how
/// it ended. The password may have changed; saying otherwise either way would
/// be a guess.
fn unknown_outcome(what: &str) -> String {
    format!("{what}; whether the password changed is not known")
}

#[cfg(test)]
mod tests {
    use super::*;
    use libauthd::transport::recv_message;
    use libauthd::wire::{
        AccessDenied, CredentialChanged, CredentialRequest, Denial, MSG_CREDENTIAL_CHANGE_START,
        MSG_CREDENTIAL_RESPONSE, decode_credential_change_start, decode_credential_response,
        encode_access_denied, encode_credential_changed, encode_credential_request,
    };
    use std::thread;

    /// Answers from a script, and remembers what it was shown.
    struct Scripted {
        answers: Vec<&'static [u8]>,
        asked: Vec<String>,
        shown: Vec<String>,
    }

    impl Scripted {
        fn new(answers: Vec<&'static [u8]>) -> Self {
            Self {
                answers,
                asked: Vec::new(),
                shown: Vec::new(),
            }
        }
    }

    impl Collector for Scripted {
        fn password(&mut self, prompt: &Prompt) -> io::Result<Secret> {
            self.asked.push(prompt.credential_name.clone());
            Ok(Secret::from_slice(self.answers.remove(0)))
        }
        fn show(&mut self, message: &Message) {
            self.shown.push(message.text.clone());
        }
    }

    fn prompt(credential_ref: u32, name: &str) -> Prompt {
        Prompt {
            credential_ref,
            credential_type: CredentialType::Password,
            credential_name: name.into(),
        }
    }

    /// Each round's answers, as `(credential_ref, data)`.
    type Answered = Vec<Vec<(u32, Vec<u8>)>>;

    /// Play the authority's side: check the opening, then send `script`,
    /// reading a response after each request.
    fn authority(socket: UnixStream, script: Vec<Vec<u8>>) -> thread::JoinHandle<Answered> {
        thread::spawn(move || {
            let opening = recv_message(&wire::FRAMING, &socket).expect("an opening");
            let (message_type, _) = decode_header(opening.expose()).expect("a header");
            assert_eq!(message_type, MSG_CREDENTIAL_CHANGE_START);
            let start = decode_credential_change_start(opening.expose()).expect("decodes");
            assert_eq!(
                start.supported_credential_types,
                vec![CredentialType::Password]
            );

            let mut answered = Vec::new();
            for message in script {
                let (message_type, _) = decode_header(&message).expect("a header");
                send_message(&socket, &message).expect("sends");
                if message_type == MSG_CREDENTIAL_REQUEST {
                    let reply = recv_message(&wire::FRAMING, &socket).expect("a response");
                    let (message_type, _) = decode_header(reply.expose()).expect("a header");
                    assert_eq!(message_type, MSG_CREDENTIAL_RESPONSE);
                    let response = decode_credential_response(reply.expose()).expect("decodes");
                    answered.push(
                        response
                            .answers
                            .iter()
                            .map(|a| (a.credential_ref, a.data.expose().to_vec()))
                            .collect(),
                    );
                }
            }
            answered
        })
    }

    fn request(messages: Vec<Message>, prompts: Vec<Prompt>) -> Vec<u8> {
        encode_credential_request(&CredentialRequest { messages, prompts }).expect("encodes")
    }

    #[test]
    fn a_change_renders_every_round_and_ends_on_changed() {
        let (client, server) = UnixStream::pair().expect("socketpair");
        let script = vec![
            request(
                vec![Message {
                    severity: MessageSeverity::Info,
                    text: "Changing the password for jack".into(),
                }],
                vec![prompt(1, "Current password")],
            ),
            request(
                Vec::new(),
                vec![prompt(2, "New password"), prompt(3, "Retype new password")],
            ),
            encode_credential_changed(&CredentialChanged).expect("encodes"),
        ];
        let authority = authority(server, script);

        let mut collector = Scripted::new(vec![b"old", b"new", b"new"]);
        assert_eq!(change(&client, &mut collector), Ok(()));

        let answered = authority.join().expect("the authority");
        assert_eq!(answered[0], vec![(1, b"old".to_vec())]);
        assert_eq!(
            answered[1],
            vec![(2, b"new".to_vec()), (3, b"new".to_vec())],
            "answers go back under the refs they were asked with"
        );
        assert_eq!(
            collector.asked,
            ["Current password", "New password", "Retype new password"]
        );
        assert_eq!(collector.shown, ["Changing the password for jack"]);
    }

    #[test]
    fn a_denial_is_reported_with_its_reason() {
        let (client, server) = UnixStream::pair().expect("socketpair");
        let script = vec![
            request(Vec::new(), vec![prompt(1, "Current password")]),
            encode_access_denied(&AccessDenied {
                denial: Denial::AuthenticationFailed,
                reason: "Authentication failed.".into(),
            })
            .expect("encodes"),
        ];
        let authority = authority(server, script);

        let outcome = change(&client, &mut Scripted::new(vec![b"guess"]));
        authority.join().expect("the authority");
        let reason = outcome.expect_err("a denial is a failure");
        assert!(reason.contains("Authentication failed."), "{reason}");
        assert!(reason.contains("unchanged"), "{reason}");
    }

    /// PGSS client obligation 4: an authority that goes away without a
    /// terminal message has not said how the change ended, and the client must
    /// not claim to know.
    #[test]
    fn an_authority_that_goes_away_leaves_the_outcome_unknown() {
        let (client, server) = UnixStream::pair().expect("socketpair");
        let authority = authority(
            server,
            vec![request(Vec::new(), vec![prompt(1, "Current password")])],
        );

        let outcome = change(&client, &mut Scripted::new(vec![b"old"]));
        authority.join().expect("the authority");
        let reason = outcome.expect_err("no terminal is not success");
        assert!(reason.contains("not known"), "{reason}");
    }

    /// A logon's terminal is not a change's. Receiving one means the authority
    /// did something this client did not ask for, and it is not success.
    #[test]
    fn a_grant_is_not_a_change() {
        let (client, server) = UnixStream::pair().expect("socketpair");
        let grant = wire::encode_access_granted(&wire::AccessGranted::default()).expect("encodes");
        let authority = authority(server, vec![grant]);

        let outcome = change(&client, &mut Scripted::new(Vec::new()));
        authority.join().expect("the authority");
        assert!(outcome.is_err());
    }
}
