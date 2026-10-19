use std::os::unix::net::{UnixListener, UnixStream};
use std::time::{Duration, Instant};

use auth_core::transport::{recv_message, send_message};
use auth_core::wire;
use peios_auth::Options;

use super::*;

fn with_authority(server: impl FnOnce(UnixStream) + Send + 'static, run: impl FnOnce(Client)) {
    let directory = tempfile::Builder::new()
        .prefix("pu-auth-")
        .tempdir_in("/tmp")
        .unwrap();
    let path = directory.path().join("socket");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("accept: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        server(stream);
    });
    run(Client::with_options(Options {
        logon_socket: path.to_str().unwrap(),
        reply_timeout_ms: 2_000,
        ..Options::default()
    })
    .unwrap());
    worker.join().unwrap();
}

fn read_start(socket: &UnixStream) {
    let bytes = recv_message(&wire::FRAMING, socket).unwrap();
    let start = wire::decode_credential_change_start(bytes.expose()).unwrap();
    assert_eq!(
        start.supported_credential_types,
        vec![wire::CredentialType::Password]
    );
}

fn request(socket: &UnixStream, references: &[u32]) {
    send_message(
        socket,
        &wire::encode_credential_request(&wire::CredentialRequest {
            messages: Vec::new(),
            prompts: references
                .iter()
                .map(|&credential_ref| wire::Prompt {
                    credential_ref,
                    credential_type: wire::CredentialType::Password,
                    credential_name: "Password".into(),
                    parameters: Vec::new(),
                })
                .collect(),
        })
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn shared_library_preserves_prompt_order_and_handles_multiple_rounds() {
    with_authority(
        |socket| {
            read_start(&socket);
            for references in [&[3, 7][..], &[][..], &[11][..]] {
                request(&socket, references);
                let bytes = recv_message(&wire::FRAMING, &socket).unwrap();
                let response = wire::decode_credential_response(bytes.expose()).unwrap();
                assert_eq!(response.answers.len(), references.len());
                for (answer, expected) in response.answers.iter().zip(references) {
                    assert_eq!(answer.credential_ref, *expected);
                    assert_eq!(answer.data.expose(), expected.to_string().as_bytes());
                }
            }
            send_message(
                &socket,
                &wire::encode_credential_changed(&wire::CredentialChanged).unwrap(),
            )
            .unwrap();
        },
        |client| {
            change_password(&client, |round| {
                Ok(round
                    .prompts
                    .iter()
                    .map(|prompt| Secret::from_slice(prompt.credential_ref.to_string().as_bytes()))
                    .collect())
            })
            .unwrap();
        },
    );
}

#[test]
fn denial_keeps_the_password_unchanged_message() {
    with_authority(
        |socket| {
            read_start(&socket);
            send_message(
                &socket,
                &wire::encode_access_denied(&wire::AccessDenied {
                    denial: Denial::AuthenticationFailed,
                    reason: "Authentication failed.".into(),
                })
                .unwrap(),
            )
            .unwrap();
        },
        |client| {
            assert_eq!(
                change_password(&client, |_| panic!("unexpected prompt")).unwrap_err(),
                "Authentication failed. The password is unchanged."
            );
        },
    );
}

#[test]
fn lost_reply_is_not_reported_as_an_unchanged_password() {
    with_authority(
        |socket| {
            read_start(&socket);
        },
        |client| {
            let error = change_password(&client, |_| panic!("unexpected prompt")).unwrap_err();
            assert!(
                error.ends_with("whether the password changed is not known"),
                "{error}"
            );
        },
    );
}

#[test]
fn input_failure_closes_the_conversation_without_sending_an_answer() {
    with_authority(
        |socket| {
            read_start(&socket);
            request(&socket, &[4]);
            assert!(recv_message(&wire::FRAMING, &socket).is_err());
        },
        |client| {
            let error = change_password(&client, |_| {
                Err("could not read the password: end of file".into())
            })
            .unwrap_err();
            assert_eq!(error, "could not read the password: end of file");
        },
    );
}

#[test]
fn denial_without_words_uses_the_protocol_name() {
    let error = Error {
        category: ERROR_PGSS_DENIAL,
        code: Denial::AccountRestricted as u32,
        outcome_unknown: false,
        message: String::new(),
    };
    assert_eq!(
        explain(&error),
        "AccountRestricted The password is unchanged."
    );
}

#[test]
fn unavailable_authority_preserves_connection_diagnostics() {
    let mut error = Error {
        category: ERROR_UNAVAILABLE,
        code: libc::ENOENT as u32,
        outcome_unknown: false,
        message: String::new(),
    };
    assert_eq!(
        explain(&error),
        "cannot reach the authority at /run/logon.sock; is authd running?"
    );
    error.code = libc::EACCES as u32;
    assert!(explain(&error).starts_with("permission denied connecting to /run/logon.sock"));
}
