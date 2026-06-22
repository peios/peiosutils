// Target resolution. Maps the user's CLI selection (--self/--real/--pid/
// --tid/--peer) onto a `peios::token::Token` with the requested access mask.
//
// pid → pidfd is an implementation detail; the user thinks in pids.

use crate::error::{Error, Result};
use peios::token::{Token, TokenAccess};
use std::os::fd::BorrowedFd;

/// What the caller selected on the command line.
#[derive(Debug, Clone, Copy)]
pub enum TargetSpec {
    /// Caller's own effective token. `real == true` opens the real
    /// (primary) token instead of the effective one when impersonating.
    SelfTok { real: bool },
    /// Process token by pid. Internally opens a pidfd.
    Pid(i32),
    /// Thread (impersonation) token. `pid` provides the pidfd, `tid`
    /// the thread id.
    Thread { pid: i32, tid: i32 },
    /// AF_UNIX peer-captured token from `sock_fd`.
    Peer(i32),
}

impl TargetSpec {
    /// Human label suitable for error messages.
    pub fn label(&self) -> String {
        match self {
            TargetSpec::SelfTok { real: false } => "self (effective)".into(),
            TargetSpec::SelfTok { real: true } => "self (real/primary)".into(),
            TargetSpec::Pid(p) => format!("pid {p}"),
            TargetSpec::Thread { pid, tid } => format!("pid {pid} tid {tid}"),
            TargetSpec::Peer(fd) => format!("peer of fd {fd}"),
        }
    }

    /// Open a `Token` against this target with the given access mask.
    /// Wraps peios errors in our `Error` enum and adds context.
    pub fn open(&self, access_mask: u32) -> Result<Token> {
        let op = self.libp_op_name();
        let access = TokenAccess::from_bits_retain(access_mask);
        let res = match *self {
            TargetSpec::SelfTok { real } => Token::open_self(real, access),
            TargetSpec::Pid(pid) => {
                let pidfd = pidfd_open(pid)?;
                // SAFETY: `pidfd` is a live, owned fd we close immediately below.
                let borrowed = unsafe { BorrowedFd::borrow_raw(pidfd) };
                let tok = Token::open_process(borrowed, access);
                // peios does not own the pidfd. Close it after the open (the
                // new token fd is independent).
                unsafe {
                    let _ = libc::close(pidfd);
                }
                tok
            }
            TargetSpec::Thread { pid, tid } => {
                let pidfd = pidfd_open(pid)?;
                // SAFETY: `pidfd` is a live, owned fd we close immediately below.
                let borrowed = unsafe { BorrowedFd::borrow_raw(pidfd) };
                let tok = Token::open_thread(borrowed, tid, access);
                unsafe {
                    let _ = libc::close(pidfd);
                }
                tok
            }
            TargetSpec::Peer(sock_fd) => {
                // SAFETY: the socket fd is owned by the caller and lives for the call.
                let borrowed = unsafe { BorrowedFd::borrow_raw(sock_fd) };
                Token::open_peer(borrowed)
            }
        };
        res.map_err(|e| {
            // Translate -EACCES to a Denied with the requested mask
            // so users get the "needed access mask" hint.
            match e.raw_os_error() {
                Some(EACCES) => Error::Denied {
                    op,
                    target: self.label(),
                    needed_mask: Some(access_mask),
                    errno: EACCES,
                },
                Some(ESRCH) | Some(ENOENT) => Error::NotFound(self.label()),
                _ => Error::from(e),
            }
        })
    }

    fn libp_op_name(&self) -> &'static str {
        match self {
            TargetSpec::SelfTok { .. } => "kacs_open_self_token",
            TargetSpec::Pid(_) => "kacs_open_process_token",
            TargetSpec::Thread { .. } => "kacs_open_thread_token",
            TargetSpec::Peer(_) => "kacs_open_peer_token",
        }
    }
}

const EACCES: i32 = 13;
const ESRCH: i32 = 3;
const ENOENT: i32 = 2;

/// Open a pidfd referring to `pid`. The returned fd is the caller's
/// responsibility to close.
fn pidfd_open(pid: i32) -> Result<i32> {
    if pid <= 0 {
        return Err(Error::Usage(format!("invalid pid: {pid}")));
    }
    // SAFETY: pidfd_open is a plain syscall taking (pid, flags).
    let rc = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if rc < 0 {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        if errno == ESRCH {
            return Err(Error::NotFound(format!("pid {pid}")));
        }
        return Err(Error::Syscall {
            op: "pidfd_open",
            errno,
            detail: Some(format!("pid {pid}")),
        });
    }
    Ok(rc as i32)
}
