// `reg watch <key>` — arm a change watch and stream notifications.
//
// LCS makes an armed key fd pollable; a readable fd means change records are
// pending. We surface each wakeup as a notification event and re-arm. The
// record *contents* are drained but not decoded here (the on-wire record layout
// is out of this tool's scope), so an event faithfully means "something under
// the filter changed — re-read", which is the contract watchers rely on.

use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{KeyAccess, NotifyFilter, OpenFlags};
use serde_json::json;
use std::os::fd::AsRawFd;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = path.display(set.sep);
    let subtree = m.get_flag("subtree");
    let filter = parse_filter(m.get_one::<String>("filter").map(String::as_str))?;
    let max = m.get_one::<u64>("count").copied();

    let key = cmd::open(&path, KeyAccess::READ, OpenFlags::empty(), &set)?;
    key.notify(filter, subtree)
        .map_err(|e| Error::from_peios("arm watch", &target, e))?;

    let fd = key.as_raw_fd();
    let mut seen = 0u64;
    loop {
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: single valid pollfd, infinite timeout.
        let r = unsafe { libc::poll(&raw mut pfd, 1, -1) };
        if r < 0 {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(5);
            if errno == libc::EINTR {
                continue;
            }
            return Err(Error::Syscall {
                op: "poll watch",
                errno,
                detail: Some(target),
            });
        }
        if pfd.revents & libc::POLLIN == 0 {
            continue;
        }
        // Drain pending record bytes (contents intentionally not decoded).
        let mut buf = [0u8; 4096];
        // SAFETY: valid fd and buffer.
        let _ = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };

        seen += 1;
        if set.json {
            println!("{}", json!({ "event": "changed", "key": target, "seq": seen }));
        } else {
            println!("changed: {target}");
        }
        if max.is_some_and(|n| seen >= n) {
            break;
        }
        // Re-arm for the next change.
        key.notify(filter, subtree)
            .map_err(|e| Error::from_peios("re-arm watch", &target, e))?;
    }
    Ok(())
}

fn parse_filter(spec: Option<&str>) -> Result<NotifyFilter> {
    let Some(spec) = spec else {
        return Ok(NotifyFilter::ALL);
    };
    let mut f = NotifyFilter::empty();
    for part in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match part {
            "value" => f |= NotifyFilter::VALUE,
            "subkey" => f |= NotifyFilter::SUBKEY,
            "sd" => f |= NotifyFilter::SD,
            "all" => f |= NotifyFilter::ALL,
            other => return Err(Error::Usage(format!("unknown watch filter: {other}"))),
        }
    }
    Ok(if f.is_empty() { NotifyFilter::ALL } else { f })
}
