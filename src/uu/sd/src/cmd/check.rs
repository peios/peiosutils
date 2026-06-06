// `sd check <path> <perms>` — access-check simulation.
//
// Builds a kacs_access_check_args via libp_sd::AccessCheckRequest, against
// either the caller's own token (default) or the named process's token.

use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::perms;
use clap::ArgMatches;
use libp_sd::{AccessCheckRequest, GenericMapping, SecurityInfo, get_sd};
use libp_token::{SelfOpenFlags, Token};
use libp_sys as sys;
use libp_token::uapi::KACS_TOKEN_QUERY;
use serde_json::json;
use std::os::fd::AsRawFd;

const SYS_PIDFD_OPEN: i64 = 434;

/// MS-DTYP file generic mapping: GENERIC_READ→FR, GENERIC_WRITE→FW,
/// GENERIC_EXECUTE→FX, GENERIC_ALL→FA. The kernel uses this to expand the
/// generic bits before evaluating the DACL.
fn file_generic_mapping() -> GenericMapping {
    GenericMapping {
        read: 0x0012_0089,
        write: 0x0012_0116,
        execute: 0x0012_00A0,
        all: 0x001F_01FF,
    }
}

pub fn run(matches: &ArgMatches) -> Result<()> {
    let target = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);
    let perms_str = matches
        .get_one::<String>("perms")
        .ok_or_else(|| Error::Usage("missing PERMS".into()))?;
    let desired = perms::parse(perms_str)?;
    let pid = matches.get_one::<i32>("pid").copied();
    let explain = matches.get_flag("explain");

    // Acquire a token fd for the check.
    let (token, token_label) = open_token(pid)?;

    // Read the SD on the path.
    let sd_bytes = get_sd(&target.as_sd_target(), SecurityInfo::all())
        .map_err(Error::from)?;
    if sd_bytes.is_empty() {
        return Err(Error::Invalid(format!(
            "{}: no SD recorded; access-check needs a descriptor",
            target.path
        )));
    }

    let req = AccessCheckRequest::new(token.as_fd().as_raw_fd(), &sd_bytes, desired)
        .mapping(file_generic_mapping());
    let decision = req
        .check()
        .map_err(|e| Error::from(e))?;

    match mode {
        OutputMode::Human => {
            println!("path:        {}", target.path);
            println!("token:       {token_label}");
            println!("desired:     0x{:08x} ({})", desired, perms::render(desired));
            println!("granted:     {}", decision.granted);
            println!(
                "granted_mask: 0x{:08x} ({})",
                decision.granted_mask,
                perms::render(decision.granted_mask)
            );
            if explain {
                if decision.granted {
                    println!("reason:      ACCESS_GRANTED (kernel)");
                } else {
                    println!("reason:      ACCESS_DENIED (kernel)");
                    println!(
                        "note:        per-ACE trace requires a kernel feature not yet wired"
                    );
                }
            }
        }
        OutputMode::Json => {
            let v = json!({
                "path": target.path,
                "token": token_label,
                "desired": format!("0x{:08x}", desired),
                "desired_rights": perms::render(desired),
                "granted": decision.granted,
                "granted_mask": format!("0x{:08x}", decision.granted_mask),
                "granted_rights": perms::render(decision.granted_mask),
            });
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
    }
    Ok(())
}

/// Open a token to run the check against. Returns the token plus a
/// human-readable label naming what was opened.
fn open_token(pid: Option<i32>) -> Result<(Token, String)> {
    if let Some(pid) = pid {
        let pidfd = unsafe { sys::syscall2(SYS_PIDFD_OPEN, pid as u64, 0) };
        if pidfd < 0 {
            return Err(Error::NotFound(format!(
                "pidfd_open(pid={pid}) failed: -E{}",
                -pidfd as i32
            )));
        }
        let pidfd = pidfd as i32;
        let token = Token::open_process(pidfd, KACS_TOKEN_QUERY).map_err(|e| {
            unsafe {
                let _ = sys::close(pidfd);
            }
            Error::Invalid(format!("open process token for pid {pid}: {e}"))
        })?;
        unsafe {
            let _ = sys::close(pidfd);
        }
        Ok((token, format!("pid {pid}")))
    } else {
        let token = Token::open_self(SelfOpenFlags::default(), KACS_TOKEN_QUERY)
            .map_err(|e| Error::Invalid(format!("open self token: {e}")))?;
        Ok((token, "self".into()))
    }
}
