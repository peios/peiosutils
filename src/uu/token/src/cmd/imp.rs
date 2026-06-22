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

use crate::cmd;
use crate::error::{Error, Result};
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::target::TargetSpec;
use peios::token::{Token, TokenAccess};
use serde_json::json;
use std::ffi::CString;
use std::os::fd::BorrowedFd;

const KACS_TOKEN_IMPERSONATE: u32 = TokenAccess::IMPERSONATE.bits();

pub fn impersonate(
    matches: &clap::ArgMatches,
    target: TargetSpec,
    mode: OutputMode,
) -> Result<()> {
    let exec_argv: Vec<String> = matches
        .get_many::<String>("exec")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();

    // Special case --peer SOCK: there is a dedicated `impersonate_peer`
    // syscall that doesn't need an explicit Token first.
    match target {
        TargetSpec::Peer(sock_fd) => {
            // SAFETY: the socket fd is owned by the caller and lives for the call.
            let borrowed = unsafe { BorrowedFd::borrow_raw(sock_fd) };
            Token::open_peer(borrowed)?.impersonate()?;
        }
        _ => {
            let tok = target.open(KACS_TOKEN_IMPERSONATE)?;
            tok.impersonate()?;
        }
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
        return cmd::emit(CmdOutput { human: lines, json: out }, mode);
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

    // Use the standard execvp: search PATH for the program. nix exposes
    // it, but for a minimal dep set we go through libc directly via
    // peios-uapi-style raw syscall. We don't have nix; use std::process
    // would lose the impersonation (it forks). So: call execvp via libc
    // FFI.
    let res = unsafe { execvp_helper(&cprog, &argv) };
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
    cmd::emit(CmdOutput { human: lines, json: json!({"status": "ok"}) }, mode)
}

// ---------------------------------------------------------------------------
// execvp via libc. We avoid pulling in `nix` for a single syscall.
// ---------------------------------------------------------------------------

unsafe fn execvp_helper(program: &CString, argv: &[CString]) -> i32 {
    let mut ptrs: Vec<*const libc::c_char> = argv.iter().map(|c| c.as_ptr()).collect();
    ptrs.push(core::ptr::null());
    let rc = unsafe { libc::execvp(program.as_ptr(), ptrs.as_ptr()) };
    if rc < 0 {
        // execvp doesn't return on success; if we got here it failed.
        std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
    } else {
        0
    }
}

// Silence the unused warning for the local Token type binding above.
#[allow(dead_code)]
fn _unused(_t: &Token) {}
