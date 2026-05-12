// peios-protoinit
//
// Throwaway PID 1 stub. Mounts core virtual filesystems, prints a banner,
// hands control to a shell. Replaced by real peinit when that exists.
//
// Built static against musl. No external crates: keeps the boot artifact
// auditable and the build trivial.

#![allow(clippy::missing_safety_doc)]

use core::ffi::c_int;
use core::ffi::c_ulong;
use std::ffi::CString;
use std::io::Write;

const MS_NODEV: c_ulong = 4;
const MS_NOEXEC: c_ulong = 8;
const MS_NOSUID: c_ulong = 2;

extern "C" {
    fn mount(
        source: *const u8,
        target: *const u8,
        fstype: *const u8,
        flags: c_ulong,
        data: *const u8,
    ) -> c_int;
    fn mkdir(path: *const u8, mode: u32) -> c_int;
    fn execve(path: *const u8, argv: *const *const u8, envp: *const *const u8) -> c_int;
    fn reboot(cmd: c_int) -> c_int;
    fn sync();
}

const RB_HALT_SYSTEM: c_int = 0xcdef0123u32 as c_int;

fn cstr(s: &str) -> CString {
    CString::new(s).expect("nul in string")
}

fn try_mount(source: &str, target: &str, fstype: &str, flags: c_ulong) {
    let s = cstr(source);
    let t = cstr(target);
    let f = cstr(fstype);
    let rc = unsafe {
        mount(
            s.as_ptr() as *const u8,
            t.as_ptr() as *const u8,
            f.as_ptr() as *const u8,
            flags,
            core::ptr::null(),
        )
    };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        eprintln!("protoinit: mount {target} ({fstype}) failed: {err}");
    }
}

fn try_mkdir(path: &str) {
    let p = cstr(path);
    unsafe {
        mkdir(p.as_ptr() as *const u8, 0o755);
    }
}

fn banner() {
    let lines = [
        "",
        "  ============================================",
        "   peios-protoinit  (PID 1 stub, v0.0.1)",
        "   throwaway scaffold; replaced by peinit",
        "  ============================================",
        "",
    ];
    let mut out = std::io::stdout().lock();
    for l in lines {
        let _ = writeln!(out, "{l}");
    }
    let _ = out.flush();
}

fn halt() -> ! {
    eprintln!("protoinit: shell exited; halting");
    unsafe {
        sync();
        reboot(RB_HALT_SYSTEM);
    }
    loop {
        std::thread::park();
    }
}

fn main() {
    try_mkdir("/proc");
    try_mkdir("/sys");
    try_mkdir("/dev");

    try_mount("proc", "/proc", "proc", MS_NOSUID | MS_NOEXEC | MS_NODEV);
    try_mount("sysfs", "/sys", "sysfs", MS_NOSUID | MS_NOEXEC | MS_NODEV);
    try_mount("devtmpfs", "/dev", "devtmpfs", MS_NOSUID);

    banner();

    let shell = cstr("/bin/sh");
    let arg0 = cstr("sh");
    let argv: [*const u8; 2] = [arg0.as_ptr() as *const u8, core::ptr::null()];
    let term = cstr("TERM=linux");
    let envp: [*const u8; 2] = [term.as_ptr() as *const u8, core::ptr::null()];

    let rc = unsafe { execve(shell.as_ptr() as *const u8, argv.as_ptr(), envp.as_ptr()) };
    let err = std::io::Error::last_os_error();
    eprintln!("protoinit: execve /bin/sh failed (rc={rc}): {err}");

    halt();
}
