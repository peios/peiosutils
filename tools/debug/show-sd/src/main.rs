// show-sd: print the KACS Security Descriptor of one or more paths.
//
// Validation tool. Calls kacs_get_sd(AT_FDCWD, path, info, ...) and decodes
// the returned self-relative SD via peios-uapi::sd.

#![allow(clippy::missing_safety_doc)]

use std::ffi::CString;
use std::os::raw::{c_long, c_void};

use peios_uapi::sd::{
    ace_flag_names, ace_type_name, access_mask_names, control_bit_names, Acl, SecurityDescriptor,
    DACL_SECURITY_INFORMATION, GROUP_SECURITY_INFORMATION, LABEL_SECURITY_INFORMATION,
    OWNER_SECURITY_INFORMATION, SACL_SECURITY_INFORMATION, SE_DACL_PRESENT, SE_SACL_PRESENT,
};
use peios_uapi::sid::Sid;
use peios_uapi::syscall::{AT_FDCWD, SYS_KACS_GET_SD};

extern "C" {
    fn syscall(num: c_long, ...) -> c_long;
    fn signal(signum: std::os::raw::c_int, handler: usize) -> usize;
}

// Rust sets SIGPIPE to SIG_IGN at startup. Restore the POSIX default so
// `cmd | head` exits silently instead of panicking out of println!.
const SIGPIPE: std::os::raw::c_int = 13;
const SIG_DFL: usize = 0;
fn restore_sigpipe_default() {
    unsafe { signal(SIGPIPE, SIG_DFL); }
}

fn get_sd(path: &CString, info: u32) -> Result<Vec<u8>, i32> {
    let need = unsafe {
        syscall(
            SYS_KACS_GET_SD,
            AT_FDCWD,
            path.as_ptr(),
            info,
            std::ptr::null::<c_void>(),
            0u32,
            0u32,
        )
    };
    if need < 0 {
        return Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(-1));
    }
    let need = need as usize;
    let mut buf = vec![0u8; need];
    if need == 0 {
        return Ok(buf);
    }
    let got = unsafe {
        syscall(
            SYS_KACS_GET_SD,
            AT_FDCWD,
            path.as_ptr(),
            info,
            buf.as_mut_ptr() as *mut c_void,
            need as u32,
            0u32,
        )
    };
    if got < 0 {
        return Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(-1));
    }
    buf.truncate(got as usize);
    Ok(buf)
}

fn print_sid_line(label: &str, sid: Option<Sid>) {
    match sid {
        Some(s) => {
            let text = s.to_string();
            println!("  {label:7}: {}{}", text, s.well_known_label());
        }
        None => println!("  {label:7}: <absent>"),
    }
}

fn print_acl(label: &str, acl: Acl<'_>) {
    println!(
        "  {label}: rev={} size={} aces={}",
        acl.revision, acl.size, acl.ace_count
    );
    for (i, ace_result) in acl.aces_iter().enumerate() {
        match ace_result {
            Ok(ace) => {
                let flag_names = ace_flag_names(ace.flags);
                let flags_str = if flag_names.is_empty() {
                    format!("{:#04x}", ace.flags)
                } else {
                    format!("{:#04x} ({})", ace.flags, flag_names.join("|"))
                };
                let detail = match ace.as_mask_sid() {
                    Some((mask, sid)) => {
                        let mask_names = access_mask_names(mask);
                        let mask_str = if mask_names.is_empty() {
                            format!("{mask:#010x}")
                        } else {
                            format!("{mask:#010x} ({})", mask_names.join("|"))
                        };
                        let sid_text = sid.to_string();
                        format!(
                            "mask={mask_str} sid={sid_text}{}",
                            sid.well_known_label()
                        )
                    }
                    None => format!("<{} bytes of body, decoder N/A>", ace.body.len()),
                };
                println!(
                    "    ace[{i}] {} flags={} size={}\n            {}",
                    ace_type_name(ace.ace_type),
                    flags_str,
                    ace.size,
                    detail
                );
            }
            Err(e) => {
                println!("    ace[{i}]: <{e}>");
                break;
            }
        }
    }
}

fn parse_and_print(buf: &[u8]) {
    let sd = match SecurityDescriptor::parse(buf) {
        Ok(s) => s,
        Err(e) => {
            println!("  <SD parse failed: {e}>");
            return;
        }
    };
    println!("  Revision : {}", sd.revision);
    println!("  Sbz1     : {}", sd.sbz1);
    let cb = control_bit_names(sd.control);
    let control_str = if cb.is_empty() {
        format!("{:#06x}", sd.control)
    } else {
        format!("{:#06x} ({})", sd.control, cb.join("|"))
    };
    println!("  Control  : {control_str}");

    print_sid_line("Owner", sd.owner());
    print_sid_line("Group", sd.group());

    match sd.dacl() {
        None => {
            if sd.control & SE_DACL_PRESENT == 0 {
                println!("  DACL   : <not present (NULL DACL implies grant-all)>");
            } else {
                println!("  DACL   : <missing>");
            }
        }
        Some(Ok(acl)) => print_acl("DACL", acl),
        Some(Err(e)) => println!("  DACL   : <{e}>"),
    }
    match sd.sacl() {
        None => {
            if sd.control & SE_SACL_PRESENT == 0 {
                println!("  SACL   : <not present>");
            } else {
                println!("  SACL   : <missing>");
            }
        }
        Some(Ok(acl)) => print_acl("SACL/label", acl),
        Some(Err(e)) => println!("  SACL   : <{e}>"),
    }
}

fn raw_hex(buf: &[u8]) {
    println!("  Raw bytes ({}):", buf.len());
    for chunk in buf.chunks(16) {
        let mut hex = String::new();
        for b in chunk {
            hex.push_str(&format!("{b:02x} "));
        }
        println!("    {hex}");
    }
}

fn main() {
    restore_sigpipe_default();
    let args: Vec<String> = std::env::args().collect();
    let mut paths: Vec<String> = Vec::new();
    let mut info = OWNER_SECURITY_INFORMATION
        | GROUP_SECURITY_INFORMATION
        | DACL_SECURITY_INFORMATION
        | LABEL_SECURITY_INFORMATION;
    let mut want_raw = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--raw" => want_raw = true,
            "--sacl" => {
                info &= !LABEL_SECURITY_INFORMATION;
                info |= SACL_SECURITY_INFORMATION;
            }
            "--no-label" => info &= !LABEL_SECURITY_INFORMATION,
            "-h" | "--help" => {
                eprintln!("usage: show-sd [--raw] [--sacl|--no-label] <path> [path...]");
                std::process::exit(0);
            }
            s if s.starts_with('-') => {
                eprintln!("show-sd: unknown flag {s}");
                std::process::exit(2);
            }
            s => paths.push(s.to_string()),
        }
        i += 1;
    }
    if paths.is_empty() {
        eprintln!("usage: show-sd [--raw] [--sacl|--no-label] <path> [path...]");
        std::process::exit(2);
    }

    for path in &paths {
        let c = CString::new(path.as_str()).unwrap_or_else(|_| {
            eprintln!("show-sd: path contains NUL: {path}");
            std::process::exit(2);
        });
        println!("=== {path} (info={info:#x}) ===");
        match get_sd(&c, info) {
            Ok(buf) => {
                if want_raw {
                    raw_hex(&buf);
                }
                parse_and_print(&buf);
            }
            Err(e) => {
                println!("  <kacs_get_sd failed: errno={e}>");
            }
        }
        println!();
    }
}
