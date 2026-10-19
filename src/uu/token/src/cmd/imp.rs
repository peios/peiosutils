// `token impersonate` and `token revert`.
//
// Without `--`, the impersonation lives only for the lifetime of the
// `token` process itself (which is rarely useful, so we warn). With
// `--`, we execve into the user's command after installing the
// impersonation token on the calling thread — analogous to
// `nice`/`taskset`/`chrt`-style state-then-exec wrappers.
//
// Narrower than `runas`: requires the target token to already exist
// (via pidfd or peer). Identity lookup is out of scope; that needs
// authd and lands later.

use std::ffi::CString;
use std::os::fd::BorrowedFd;

use peios::token::{Token, TokenAccess};
use serde_json::json;

use crate::cmd;
use crate::error::{Error, Result};
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::target::TargetSpec;

const KACS_TOKEN_IMPERSONATE: u32 = TokenAccess::IMPERSONATE.bits();

pub fn impersonate(matches: &clap::ArgMatches, target: TargetSpec, mode: OutputMode) -> Result<()> {
    let exec_argv: Vec<String> = matches
        .get_many::<String>("exec")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();

    // Peer tokens are opened through their socket; other targets request
    // explicit impersonation access when resolving the token.
    if let TargetSpec::Peer(sock_fd) = target {
        // SAFETY: the socket fd is owned by the caller and lives for the call.
        let borrowed = unsafe { BorrowedFd::borrow_raw(sock_fd) };
        Token::open_peer(borrowed)?.impersonate()?;
    } else {
        let tok = target.open(KACS_TOKEN_IMPERSONATE)?;
        tok.impersonate()?;
    }

    if exec_argv.is_empty() {
        let mut lines = Lines::new();
        lines.section("impersonate");
        lines.kv("target", target.label());
        lines.detail(
            "WARNING: impersonation will be dropped when this process exits; \
             pass `-- <cmd>` to exec under the impersonating context.",
        );
        let out = json!({
            "target": target.label(),
            "exec": serde_json::Value::Null,
            "warning": "impersonation dropped at process exit",
        });
        return cmd::emit(
            CmdOutput {
                human: lines,
                json: out,
            },
            mode,
        );
    }

    // Exec form: replace the process. From here we don't return.
    let program = exec_argv[0].clone();
    let argv: Vec<CString> = exec_argv
        .iter()
        .map(|s| CString::new(s.as_str()))
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| Error::Usage("argv contained an internal NUL byte".into()))?;
    let cprog = CString::new(program.as_str())
        .map_err(|_| Error::Usage("program path contained a NUL byte".into()))?;

    // Replace this process through libc so the program keeps this thread's
    // impersonation state. execvp searches PATH for an unqualified program name.
    let res = execvp_helper(&cprog, &argv);
    Err(Error::Syscall {
        op: "execvp",
        errno: res,
        detail: Some(program),
    })
}

pub fn revert(mode: OutputMode) -> Result<()> {
    Token::revert()?;
    let mut lines = Lines::new();
    lines.section("revert");
    lines.kv("status", "ok");
    cmd::emit(
        CmdOutput {
            human: lines,
            json: json!({"status": "ok"}),
        },
        mode,
    )
}

// ---------------------------------------------------------------------------
// execvp via libc. We avoid pulling in `nix` for a single syscall.
// ---------------------------------------------------------------------------

/// Replace this process, returning the syscall error number on failure.
fn execvp_helper(program: &CString, argv: &[CString]) -> i32 {
    let mut ptrs: Vec<*const libc::c_char> = argv.iter().map(|c| c.as_ptr()).collect();
    ptrs.push(core::ptr::null());
    // SAFETY: CString owns every terminated string, and ptrs adds the required
    // null terminator. Both the strings and pointer array remain alive for the call.
    let rc = unsafe { libc::execvp(program.as_ptr(), ptrs.as_ptr()) };
    if rc < 0 {
        // execvp doesn't return on success; if we got here it failed.
        std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
    } else {
        0
    }
}
