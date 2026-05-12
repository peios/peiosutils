// whoami-token: dump the calling process's KACS access token.
//
// Validation tool. Opens the calling task's primary token via the
// kacs_open_self_token syscall, then iterates KACS_IOC_QUERY across the
// well-known TOKEN_CLASS_* classes and prints their decoded contents.
//
// Static musl binary. All constants/types come from peios-uapi.

#![allow(clippy::missing_safety_doc)]

use std::os::raw::{c_int, c_long, c_ulong, c_void};

use peios_uapi::sid::Sid;
use peios_uapi::syscall::SYS_KACS_OPEN_SELF_TOKEN;
use peios_uapi::token::{
    elevation_type_name, impersonation_level_name, token_type_name, KacsQueryArgs, KACS_IOC_QUERY,
    KACS_TOKEN_QUERY, PRIVILEGES, TOKEN_CLASS_ELEVATION_TYPE, TOKEN_CLASS_GROUPS,
    TOKEN_CLASS_INTEGRITY_LEVEL, TOKEN_CLASS_OWNER, TOKEN_CLASS_PRIMARY_GROUP,
    TOKEN_CLASS_PRIVILEGES, TOKEN_CLASS_SESSION_ID, TOKEN_CLASS_STATISTICS, TOKEN_CLASS_TYPE,
    TOKEN_CLASS_USER,
};

extern "C" {
    fn syscall(num: c_long, ...) -> c_long;
    fn ioctl(fd: c_int, req: c_ulong, ...) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn signal(signum: c_int, handler: usize) -> usize;
}

// Rust sets SIGPIPE to SIG_IGN at startup, which turns "writing to a closed
// pipe" into an EPIPE return that println! panics on. Restore the POSIX
// default so `cmd | head` exits silently like every other Unix tool.
const SIGPIPE: c_int = 13;
const SIG_DFL: usize = 0;
fn restore_sigpipe_default() {
    unsafe { signal(SIGPIPE, SIG_DFL); }
}

fn die(stage: &str) -> ! {
    let err = std::io::Error::last_os_error();
    eprintln!("whoami-token: {stage}: {err}");
    std::process::exit(1);
}

fn open_self_token() -> c_int {
    let fd = unsafe { syscall(SYS_KACS_OPEN_SELF_TOKEN, 0u32, KACS_TOKEN_QUERY) };
    if fd < 0 {
        die("kacs_open_self_token");
    }
    fd as c_int
}

fn query(fd: c_int, class: u32) -> Result<Vec<u8>, i32> {
    let mut args = KacsQueryArgs {
        token_class: class,
        buf_len: 0,
        buf_ptr: 0,
    };
    let rc = unsafe { ioctl(fd, KACS_IOC_QUERY as c_ulong, &mut args as *mut _ as *mut c_void) };
    if rc < 0 {
        return Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(-1));
    }
    let need = args.buf_len as usize;
    let mut buf = vec![0u8; need];
    if need > 0 {
        args.buf_ptr = buf.as_mut_ptr() as u64;
        let rc = unsafe { ioctl(fd, KACS_IOC_QUERY as c_ulong, &mut args as *mut _ as *mut c_void) };
        if rc < 0 {
            return Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(-1));
        }
    }
    Ok(buf)
}

fn try_query(fd: c_int, label: &str, class: u32) -> Option<Vec<u8>> {
    match query(fd, class) {
        Ok(v) => Some(v),
        Err(e) => {
            println!("{label:18}: <query failed: errno={e}>");
            None
        }
    }
}

fn print_sid_field(label: &str, bytes: &[u8]) {
    match Sid::parse(bytes) {
        Ok((sid, _)) => {
            let s = sid.to_string();
            println!("{label:18}: {}{}", s, sid.well_known_label());
        }
        Err(e) => println!("{label:18}: <invalid sid: {e}>"),
    }
}

fn dump_privileges(b: &[u8]) {
    if b.len() < 32 {
        println!("  <invalid privileges payload: {} bytes>", b.len());
        return;
    }
    let present = u64::from_le_bytes(b[0..8].try_into().unwrap());
    let enabled = u64::from_le_bytes(b[8..16].try_into().unwrap());
    let default_enabled = u64::from_le_bytes(b[16..24].try_into().unwrap());
    let used = u64::from_le_bytes(b[24..32].try_into().unwrap());
    println!("  present         : {present:#018x}");
    println!("  enabled         : {enabled:#018x}");
    println!("  default_enabled : {default_enabled:#018x}");
    println!("  used            : {used:#018x}");
    let mut any = false;
    for &(bit, name) in PRIVILEGES {
        let mask = 1u64 << bit;
        if present & mask == 0 {
            continue;
        }
        let mut flags = Vec::new();
        if enabled & mask != 0 {
            flags.push("enabled");
        }
        if default_enabled & mask != 0 {
            flags.push("default");
        }
        if used & mask != 0 {
            flags.push("used");
        }
        let tag = if flags.is_empty() {
            "(present)".to_string()
        } else {
            flags.join(",")
        };
        println!("  {name:30}  {tag}");
        any = true;
    }
    if !any {
        println!("  (no privileges)");
    }
}

fn u32_le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn u64_le(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}

fn main() {
    restore_sigpipe_default();
    let fd = open_self_token();

    println!("=== whoami-token ===");

    if let Some(b) = try_query(fd, "User", TOKEN_CLASS_USER) {
        print_sid_field("User", &b);
    }
    if let Some(b) = try_query(fd, "Owner", TOKEN_CLASS_OWNER) {
        print_sid_field("Owner", &b);
    }
    if let Some(b) = try_query(fd, "Primary group", TOKEN_CLASS_PRIMARY_GROUP) {
        print_sid_field("Primary group", &b);
    }
    if let Some(b) = try_query(fd, "Integrity level", TOKEN_CLASS_INTEGRITY_LEVEL) {
        print_sid_field("Integrity level", &b);
    }
    if let Some(b) = try_query(fd, "Token type", TOKEN_CLASS_TYPE) {
        if b.len() == 4 {
            let v = u32_le(&b, 0);
            println!("{:18}: {} ({v})", "Token type", token_type_name(v));
        }
    }
    if let Some(b) = try_query(fd, "Elevation", TOKEN_CLASS_ELEVATION_TYPE) {
        if b.len() == 4 {
            let v = u32_le(&b, 0);
            println!("{:18}: {} ({v})", "Elevation", elevation_type_name(v));
        }
    }
    if let Some(b) = try_query(fd, "Session ID", TOKEN_CLASS_SESSION_ID) {
        if b.len() == 4 {
            println!("{:18}: {}", "Session ID", u32_le(&b, 0));
        }
    }
    if let Some(b) = try_query(fd, "Statistics", TOKEN_CLASS_STATISTICS) {
        if b.len() == 40 {
            println!("{:18}: {}", "Token ID", u64_le(&b, 0));
            println!("{:18}: {}", "Auth session", u64_le(&b, 8));
            println!("{:18}: {}", "Modified ID", u64_le(&b, 16));
            println!("{:18}: {}", "Stats.type", u32_le(&b, 24));
            println!("{:18}: {}", "Expiration", u64_le(&b, 32));
        }
    }
    if let Some(b) = try_query(fd, "Groups", TOKEN_CLASS_GROUPS) {
        println!("{:18}: {} bytes (not decoded)", "Groups payload", b.len());
    }

    println!();
    println!("Privileges:");
    if let Some(b) = try_query(fd, "  privileges", TOKEN_CLASS_PRIVILEGES) {
        dump_privileges(&b);
    }

    // Silences "unused import" if impersonation_level_name happens to not be
    // exercised on this token (we don't currently query it).
    let _ = impersonation_level_name;

    unsafe {
        close(fd);
    }
}
