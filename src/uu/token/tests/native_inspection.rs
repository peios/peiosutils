//! Run explicitly on a disposable Peios kernel with its initial SYSTEM token:
//! TOKEN_TEST_BINARY=/path/to/token cargo test -p pu_token --test native_inspection -- --ignored
use peios::security::Privileges;
use peios::token::{
    ImpersonationLevel, PrivilegeAdjustment, RestrictSpec, Token, TokenAccess, TokenType,
};
use serde_json::Value;
use std::process::{Command, Output};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn command(args: &[&str]) -> Output {
    Command::new(
        std::env::var_os("TOKEN_TEST_BINARY").unwrap_or_else(|| env!("CARGO_BIN_EXE_token").into()),
    )
    .args(args)
    .output()
    .expect("run token command")
}

fn json(args: &[&str]) -> Value {
    let out = command(args);
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("one JSON result")
}

struct Worker {
    stop: Option<mpsc::Sender<()>>,
    join: Option<thread::JoinHandle<()>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Unblock the worker even when an assertion unwinds.
        self.stop.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[test]
#[ignore = "requires a disposable Peios kernel with the initial SYSTEM token"]
fn compare_and_threads_observe_real_impersonation_and_privilege_differences() {
    let primary = Token::open_self(true, TokenAccess::QUERY | TokenAccess::DUPLICATE).unwrap();
    let user = primary.user().unwrap().to_string();
    let privileges = primary.privileges().unwrap();
    assert!(
        privileges
            .enabled
            .contains(Privileges::SHUTDOWN | Privileges::REMOTE_SHUTDOWN)
    );
    let shutdown = Privileges::SHUTDOWN.bits().trailing_zeros();
    let remote_shutdown = Privileges::REMOTE_SHUTDOWN.bits().trailing_zeros();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (stop_tx, stop_rx) = mpsc::channel();
    let join = thread::spawn(move || {
        let real = Token::open_self(true, TokenAccess::QUERY | TokenAccess::DUPLICATE).unwrap();
        let admin = real
            .groups()
            .unwrap()
            .iter()
            .position(|(sid, _)| sid.to_string() == "S-1-5-32-544")
            .expect("initial administrator group");
        let filtered = real
            .restrict(&RestrictSpec {
                deny_group_indices: vec![admin as u32],
                ..RestrictSpec::default()
            })
            .unwrap();
        let impersonation = filtered
            .duplicate(
                TokenAccess::ALL_ACCESS,
                TokenType::Impersonation,
                ImpersonationLevel::Impersonation,
            )
            .unwrap();
        impersonation
            .adjust_privileges(&[
                PrivilegeAdjustment::remove(shutdown),
                PrivilegeAdjustment::disable(remote_shutdown),
            ])
            .unwrap();
        let _guard = impersonation.impersonate_scoped().unwrap();
        // SAFETY: gettid has no arguments and only observes this thread.
        ready_tx
            .send(unsafe { libc::syscall(libc::SYS_gettid) } as i32)
            .unwrap();
        let _ = stop_rx.recv_timeout(Duration::from_secs(30));
    });
    let _worker = Worker {
        stop: Some(stop_tx),
        join: Some(join),
    };
    let tid = ready_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("impersonating worker ready");
    let pid = std::process::id().to_string();
    let tid_string = tid.to_string();

    let report = json(&["threads", &pid, "--json"]);
    assert_eq!(report["complete"], true);
    let rows = report["threads"].as_array().unwrap();
    assert!(
        rows.windows(2)
            .all(|pair| pair[0]["tid"].as_i64() < pair[1]["tid"].as_i64())
    );
    let worker = rows
        .iter()
        .find(|row| row["tid"] == tid)
        .expect("worker listed");
    assert_eq!(worker["impersonating"], true);
    assert_eq!(worker["user"]["sid"], user);
    let main = rows
        .iter()
        .find(|row| row["tid"] == std::process::id())
        .expect("main thread listed");
    assert_eq!(main["impersonating"], false);
    assert_eq!(main["user"]["sid"], user);

    let comparison = json(&[
        "compare",
        &pid,
        "--pid",
        &pid,
        "--tid",
        &tid_string,
        "--json",
    ]);
    for field in ["user", "integrity", "session_id"] {
        assert_eq!(comparison["header"][field]["matches"], true, "{field}");
    }
    assert_eq!(comparison["header"]["user"]["left"]["sid"], user);
    let rows = comparison["privileges"].as_array().unwrap();
    let removed = rows.iter().find(|row| row["bit"] == shutdown).unwrap();
    assert_eq!(removed["left"], "enabled");
    assert!(removed["right"].is_null());
    assert_eq!(removed["matches"], false);
    let disabled = rows
        .iter()
        .find(|row| row["bit"] == remote_shutdown)
        .unwrap();
    assert_eq!(disabled["left"], "enabled");
    assert_eq!(disabled["right"], "disabled");
    assert_eq!(disabled["matches"], false);
    let admin = comparison["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["group"]["sid"] == "S-1-5-32-544")
        .unwrap();
    assert_eq!(admin["left"], "enabled");
    assert_eq!(admin["right"], "deny-only");
    assert_eq!(admin["matches"], false);

    let human = command(&["compare", &pid, "--pid", &pid, "--tid", &tid_string]);
    assert!(human.status.success());
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(
        text.contains("!=") && text.contains("SeShutdown") && text.contains("Local System"),
        "{text}"
    );
    let threads = command(&["threads", &pid]);
    assert!(threads.status.success());
    let text = String::from_utf8(threads.stdout).unwrap();
    assert!(
        text.contains("yes") && text.contains("no") && text.contains(&user),
        "{text}"
    );

    // The task's two-PID and effective-versus-primary forms also run end to end.
    for args in [
        vec!["compare", &pid, "1", "--json"],
        vec!["compare", "self", "--real", "--json"],
    ] {
        let result = json(&args);
        assert_eq!(result["header"]["user"]["matches"], true);
    }
    let missing = command(&["threads", "2147483647", "--json"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());
}
