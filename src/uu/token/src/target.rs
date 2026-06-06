// Target resolution. Maps the user's CLI selection (--self/--real/--pid/
// --tid/--peer) onto a `libp_token::Token` with the requested access mask.
//
// pid → pidfd is an implementation detail; the user thinks in pids.

use crate::error::{Error, Result};
use libp_token::{SelfOpenFlags, Token};
use libp_sys as sys;

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
    /// Wraps libp-token errors in our `Error` enum and adds context.
    pub fn open(&self, access_mask: u32) -> Result<Token> {
        let op = self.libp_op_name();
        let res = match *self {
            TargetSpec::SelfTok { real } => {
                Token::open_self(SelfOpenFlags { real_token: real }, access_mask)
            }
            TargetSpec::Pid(pid) => {
                let pidfd = pidfd_open(pid)?;
                let tok = Token::open_process(pidfd, access_mask);
                // libp-token does not own the pidfd. Close it after the
                // open (the new token fd is independent).
                unsafe {
                    let _ = sys::close(pidfd);
                }
                tok
            }
            TargetSpec::Thread { pid, tid } => {
                let pidfd = pidfd_open(pid)?;
                let tok = Token::open_thread(pidfd, tid, access_mask);
                unsafe {
                    let _ = sys::close(pidfd);
                }
                tok
            }
            TargetSpec::Peer(sock_fd) => Token::open_peer(sock_fd),
        };
        res.map_err(|e| {
            // Translate -EACCES to a Denied with the requested mask
            // so users get the "needed access mask" hint.
            if let libp_token::Error::Syscall(errno) = &e {
                if errno.raw() == EACCES {
                    return Error::Denied {
                        op,
                        target: self.label(),
                        needed_mask: Some(access_mask),
                        errno: EACCES,
                    };
                }
                if errno.raw() == ESRCH || errno.raw() == ENOENT {
                    return Error::NotFound(self.label());
                }
            }
            Error::from(e)
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

// SYS_pidfd_open on x86_64. Matches libp-test's helper.
const SYS_PIDFD_OPEN: i64 = 434;

/// Open a pidfd referring to `pid`. The returned fd is the caller's
/// responsibility to close.
fn pidfd_open(pid: i32) -> Result<i32> {
    if pid <= 0 {
        return Err(Error::Usage(format!("invalid pid: {pid}")));
    }
    let rc = unsafe { sys::syscall2(SYS_PIDFD_OPEN, pid as u64, 0) };
    if rc < 0 {
        let errno = -rc as i32;
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
