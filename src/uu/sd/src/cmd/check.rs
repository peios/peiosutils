// `sd check <path> <perms>` — access-check simulation.
//
// Runs the KACS AccessCheck pipeline (via `peios::access::AccessCheck`) against
// either the caller's own token (default) or the named process's token.

use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::perms;
use clap::ArgMatches;
use peios::access::AccessCheck;
use peios::file::{SecInfo, get_sd};
use peios::security::{AccessMask, GenericMapping};
use peios::token::{Token, TokenAccess};
use serde_json::json;
use std::os::fd::{AsFd, FromRawFd, OwnedFd};

/// MS-DTYP file generic mapping: GENERIC_READ→FR, GENERIC_WRITE→FW,
/// GENERIC_EXECUTE→FX, GENERIC_ALL→FA. The kernel uses this to expand the
/// generic bits before evaluating the DACL.
fn file_generic_mapping() -> GenericMapping {
    GenericMapping::new(0x0012_0089, 0x0012_0116, 0x0012_00A0, 0x001F_01FF)
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

    // Acquire a token for the check.
    let (token, token_label) = open_token(pid)?;

    // Read the SD on the path.
    let all = SecInfo::OWNER | SecInfo::GROUP | SecInfo::DACL | SecInfo::SACL | SecInfo::LABEL;
    let sd = get_sd(target.dirfd(), target.as_path(), all, target.at_flags()).map_err(Error::from)?;
    if sd.as_bytes().is_empty() {
        return Err(Error::Invalid(format!(
            "{}: no SD recorded; access-check needs a descriptor",
            target.path
        )));
    }

    let mut check = AccessCheck::new(
        &sd,
        AccessMask::from_bits_retain(desired),
        file_generic_mapping(),
    );
    check.token(token.as_fd());
    let decision = check.check().map_err(Error::from)?;
    let granted_mask = decision.granted.bits();

    match mode {
        OutputMode::Human => {
            println!("path:        {}", target.path);
            println!("token:       {token_label}");
            println!("desired:     0x{:08x} ({})", desired, perms::render(desired));
            println!("granted:     {}", decision.allowed);
            println!(
                "granted_mask: 0x{:08x} ({})",
                granted_mask,
                perms::render(granted_mask)
            );
            if explain {
                if decision.allowed {
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
                "granted": decision.allowed,
                "granted_mask": format!("0x{:08x}", granted_mask),
                "granted_rights": perms::render(granted_mask),
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
        // `libp-sys` is gone; open the pidfd directly via libc.
        let pidfd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        if pidfd < 0 {
            let e = std::io::Error::last_os_error();
            return Err(Error::NotFound(format!(
                "pidfd_open(pid={pid}) failed: {e}"
            )));
        }
        // SAFETY: pidfd_open returned a fresh, owned fd.
        let pidfd: OwnedFd = unsafe { OwnedFd::from_raw_fd(pidfd as i32) };
        let token = Token::open_process(pidfd.as_fd(), TokenAccess::QUERY)
            .map_err(|e| Error::Invalid(format!("open process token for pid {pid}: {e}")))?;
        Ok((token, format!("pid {pid}")))
    } else {
        let token = Token::open_self(false, TokenAccess::QUERY)
            .map_err(|e| Error::Invalid(format!("open self token: {e}")))?;
        Ok((token, "self".into()))
    }
}
