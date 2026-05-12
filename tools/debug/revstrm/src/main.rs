// revstrm: KMES ring inspector / emitter. Subcommands:
//   emit    — write one event via syscall(kmes_emit)
//   dump    — attach all per-CPU rings, drain pending events, exit
//   follow  — like dump, then keep polling
//
// Static musl binary. Wire-format constants and the msgpack codec come from
// peios-uapi; this binary keeps the raw mmap / syscall plumbing.

#![allow(clippy::missing_safety_doc)]

use std::ffi::CString;
use std::io::Write;
use std::os::raw::{c_int, c_long, c_void};

use peios_uapi::kmes::{
    origin_name, ring_mapping_size, EventHeader, HDR_BASE, P_CAPACITY, P_CPU_ID, P_MAGIC,
    P_TAIL_POS, P_WRITE_POS, RING_MAGIC,
};
use peios_uapi::msgpack::{
    self, encode_array_prefix, encode_int, encode_map_prefix, encode_str, encode_uint, Value, NIL,
};
use peios_uapi::syscall::{SYS_KMES_ATTACH, SYS_KMES_EMIT};

const PROT_READ: c_int = 1;
const PROT_WRITE: c_int = 2;
const MAP_SHARED: c_int = 1;
const MAP_FAILED: isize = -1;

extern "C" {
    fn syscall(num: c_long, ...) -> c_long;
    fn mmap(
        addr: *mut c_void,
        len: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        off: i64,
    ) -> *mut c_void;
    fn munmap(addr: *mut c_void, len: usize) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn nanosleep(req: *const Timespec, rem: *mut Timespec) -> c_int;
    fn signal(signum: c_int, handler: usize) -> usize;
}

// Rust sets SIGPIPE to SIG_IGN at startup. Restore the POSIX default so
// `cmd | head` exits silently instead of panicking out of println!.
const SIGPIPE: c_int = 13;
const SIG_DFL: usize = 0;
fn restore_sigpipe_default() {
    unsafe { signal(SIGPIPE, SIG_DFL); }
}

#[repr(C)]
struct Timespec {
    sec: c_long,
    nsec: c_long,
}

fn die(stage: &str) -> ! {
    let err = std::io::Error::last_os_error();
    eprintln!("revstrm: {stage}: {err}");
    std::process::exit(1);
}

fn sleep_ms(ms: u64) {
    let ts = Timespec {
        sec: (ms / 1000) as c_long,
        nsec: ((ms % 1000) * 1_000_000) as c_long,
    };
    unsafe {
        nanosleep(&ts, std::ptr::null_mut());
    }
}

// ============================================================================
// emit
// ============================================================================

enum EmitMode {
    Single(Option<String>),
    Map(Vec<String>),
    Array(Vec<String>),
    Raw(String),
}

fn cmd_emit(args: &[String]) -> ! {
    if args.is_empty() {
        eprintln!(
            "usage:\n  \
             revstrm emit <event_type> [payload]              # str payload\n  \
             revstrm emit <event_type> --map k=v [k=v ...]    # str->str map\n  \
             revstrm emit <event_type> --array v [v ...]      # str array\n  \
             revstrm emit <event_type> --raw <hex|bytes>      # raw msgpack bytes\n\
             values in --map / --array may carry an explicit type tag:\n  \
             int:42  uint:42  bool:true  str:hello  nil:"
        );
        std::process::exit(2);
    }

    let mut event_type: Option<&str> = None;
    let mut mode = EmitMode::Single(None);
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--raw" => {
                if i + 1 >= args.len() {
                    eprintln!("revstrm emit: --raw needs a hex string");
                    std::process::exit(2);
                }
                mode = EmitMode::Raw(args[i + 1].clone());
                i += 2;
            }
            "--map" => {
                let rest = &args[i + 1..];
                mode = EmitMode::Map(rest.to_vec());
                i = args.len();
            }
            "--array" => {
                let rest = &args[i + 1..];
                mode = EmitMode::Array(rest.to_vec());
                i = args.len();
            }
            _ if event_type.is_none() => {
                event_type = Some(a.as_str());
                i += 1;
            }
            _ => {
                if let EmitMode::Single(ref mut p) = mode {
                    *p = Some(a.clone());
                    i += 1;
                } else {
                    eprintln!("revstrm emit: unexpected arg {a:?}");
                    std::process::exit(2);
                }
            }
        }
    }

    let event_type = event_type.unwrap_or_else(|| {
        eprintln!("revstrm emit: missing event_type");
        std::process::exit(2);
    });
    let event_type_b = event_type.as_bytes();

    let payload: Vec<u8> = match mode {
        EmitMode::Single(None) => vec![NIL],
        EmitMode::Single(Some(s)) => encode_str(s.as_bytes()),
        EmitMode::Raw(hex) => parse_hex_or_text(&hex),
        EmitMode::Map(pairs) => encode_map_payload(&pairs),
        EmitMode::Array(items) => encode_array_payload(&items),
    };

    if event_type_b.is_empty() || event_type_b.len() > u16::MAX as usize {
        eprintln!("revstrm emit: event_type length out of range");
        std::process::exit(2);
    }
    if payload.len() > u32::MAX as usize {
        eprintln!("revstrm emit: payload too large");
        std::process::exit(2);
    }

    let rc = unsafe {
        syscall(
            SYS_KMES_EMIT,
            event_type_b.as_ptr() as *const c_void,
            event_type_b.len() as std::os::raw::c_uint,
            payload.as_ptr() as *const c_void,
            payload.len() as std::os::raw::c_uint,
        )
    };
    if rc < 0 {
        die("kmes_emit");
    }
    println!("emit ok: type={:?} payload={} bytes", event_type, payload.len());
    std::process::exit(0);
}

// Parse a value of the form "[type:]content"; accepted tags: str, int, uint,
// bool, nil. Defaults to str.
fn encode_tagged_value(s: &str) -> Vec<u8> {
    let (tag, rest) = match s.split_once(':') {
        Some((t, r)) if matches!(t, "str" | "int" | "uint" | "bool" | "nil") => (t, r),
        _ => ("str", s),
    };
    match tag {
        "nil" => vec![NIL],
        "bool" => match rest {
            "true" | "1" | "yes" => vec![0xc3],
            _ => vec![0xc2],
        },
        "uint" => match rest.parse::<u64>() {
            Ok(v) => encode_uint(v),
            Err(_) => encode_str(rest.as_bytes()),
        },
        "int" => match rest.parse::<i64>() {
            Ok(v) => encode_int(v),
            Err(_) => encode_str(rest.as_bytes()),
        },
        _ => encode_str(rest.as_bytes()),
    }
}

fn encode_map_payload(pairs: &[String]) -> Vec<u8> {
    let mut out = encode_map_prefix(pairs.len());
    for kv in pairs {
        let (k, v) = match kv.split_once('=') {
            Some(p) => p,
            None => {
                eprintln!("revstrm emit: --map entry {kv:?} missing '='");
                std::process::exit(2);
            }
        };
        out.extend_from_slice(&encode_str(k.as_bytes()));
        out.extend_from_slice(&encode_tagged_value(v));
    }
    out
}

fn encode_array_payload(items: &[String]) -> Vec<u8> {
    let mut out = encode_array_prefix(items.len());
    for v in items {
        out.extend_from_slice(&encode_tagged_value(v));
    }
    out
}

fn parse_hex_or_text(s: &str) -> Vec<u8> {
    if let Some(hex) = s.strip_prefix("0x") {
        let mut out = Vec::with_capacity(hex.len() / 2);
        let chars: Vec<char> = hex.chars().filter(|c| !c.is_whitespace()).collect();
        let mut i = 0;
        while i + 1 < chars.len() {
            let pair: String = chars[i..i + 2].iter().collect();
            match u8::from_str_radix(&pair, 16) {
                Ok(b) => out.push(b),
                Err(_) => {
                    eprintln!("revstrm emit: bad hex byte {pair:?}");
                    std::process::exit(2);
                }
            }
            i += 2;
        }
        out
    } else {
        s.as_bytes().to_vec()
    }
}

// ============================================================================
// attach + ring iteration
// ============================================================================

struct Ring {
    fd: c_int,
    map: *mut u8,
    map_len: usize,
    capacity: u64,
    data: *mut u8,
    producer: *mut u8,
    cpu_id: u16,
    cursor: u64,
}

impl Drop for Ring {
    fn drop(&mut self) {
        unsafe {
            if !self.map.is_null() {
                munmap(self.map as *mut c_void, self.map_len);
            }
            if self.fd >= 0 {
                close(self.fd);
            }
        }
    }
}

fn attach() -> Vec<Ring> {
    let mut count: c_int = 0;
    let mut capacity: u64 = 0;
    let rc = unsafe {
        syscall(
            SYS_KMES_ATTACH,
            std::ptr::null_mut::<c_int>(),
            &mut count as *mut c_int,
            &mut capacity as *mut u64,
        )
    };
    if rc < 0 {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1);
        // ERANGE on probe is expected.
        if errno != 34 {
            die("kmes_attach (probe)");
        }
    }
    if count <= 0 {
        eprintln!("revstrm: kmes_attach reports 0 CPUs (KMES not ready?)");
        std::process::exit(1);
    }

    let mut fds: Vec<c_int> = vec![-1; count as usize];
    let rc = unsafe {
        syscall(
            SYS_KMES_ATTACH,
            fds.as_mut_ptr(),
            &mut count as *mut c_int,
            &mut capacity as *mut u64,
        )
    };
    if rc < 0 {
        die("kmes_attach");
    }

    let mut rings = Vec::with_capacity(fds.len());
    for &fd in &fds {
        if fd < 0 {
            continue;
        }
        let map_len = ring_mapping_size(capacity);
        let p = unsafe {
            mmap(
                std::ptr::null_mut(),
                map_len,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                fd,
                0,
            )
        };
        if p as isize == MAP_FAILED {
            die("mmap ring");
        }
        let producer = p as *mut u8;
        let data = unsafe { producer.add(2 * peios_uapi::kmes::PAGE_SIZE) };

        let magic = unsafe { std::slice::from_raw_parts(producer.add(P_MAGIC), 8) };
        if magic != RING_MAGIC {
            eprintln!("revstrm: bad ring magic on fd {fd}: got {magic:?} want {RING_MAGIC:?}");
            continue;
        }
        let cpu_id = unsafe { *(producer.add(P_CPU_ID) as *const u16) };
        let ring_capacity = unsafe { *(producer.add(P_CAPACITY) as *const u64) };
        if ring_capacity != capacity {
            eprintln!(
                "revstrm: ring {cpu_id} capacity mismatch: producer says {ring_capacity}, attach says {capacity}"
            );
        }
        let tail = unsafe { *(producer.add(P_TAIL_POS) as *const u64) };
        rings.push(Ring {
            fd,
            map: producer,
            map_len,
            capacity,
            data,
            producer,
            cpu_id,
            cursor: tail,
        });
    }
    rings
}

fn ring_read(r: &Ring, pos: u64, len: usize) -> Vec<u8> {
    let off = (pos % r.capacity) as usize;
    let mut out = vec![0u8; len];
    unsafe {
        std::ptr::copy_nonoverlapping(r.data.add(off), out.as_mut_ptr(), len);
    }
    out
}

fn ring_write_pos(r: &Ring) -> u64 {
    unsafe { std::ptr::read_volatile(r.producer.add(P_WRITE_POS) as *const u64) }
}

fn drain_ring(r: &mut Ring) -> usize {
    let write_pos = ring_write_pos(r);
    if write_pos == r.cursor {
        return 0;
    }
    if write_pos < r.cursor {
        eprintln!(
            "revstrm: cpu {} write_pos {} < cursor {}, resyncing",
            r.cpu_id, write_pos, r.cursor
        );
        r.cursor = write_pos;
        return 0;
    }
    if write_pos - r.cursor > r.capacity {
        let lost = (write_pos - r.cursor) - r.capacity;
        eprintln!(
            "revstrm: cpu {} overrun: {} bytes lost, snapping cursor",
            r.cpu_id, lost
        );
        let kernel_tail = unsafe { *(r.producer.add(P_TAIL_POS) as *const u64) };
        r.cursor = kernel_tail;
    }

    let mut n = 0;
    while r.cursor < write_pos {
        let head4 = ring_read(r, r.cursor, 4);
        let event_size = u32::from_le_bytes(head4.as_slice().try_into().unwrap()) as u64;
        if event_size < HDR_BASE as u64 || event_size > r.capacity {
            eprintln!(
                "revstrm: cpu {} invalid event_size {} at pos {}, resyncing to write_pos",
                r.cpu_id, event_size, r.cursor
            );
            r.cursor = write_pos;
            break;
        }
        if r.cursor + event_size > write_pos {
            break;
        }
        let bytes = ring_read(r, r.cursor, event_size as usize);
        print_event(&bytes);
        r.cursor += event_size;
        n += 1;
    }
    n
}

fn print_event(b: &[u8]) {
    let hdr = match EventHeader::parse(b) {
        Ok(h) => h,
        Err(e) => {
            println!("    <header parse failed: {e}>");
            return;
        }
    };
    let type_str = std::str::from_utf8(hdr.event_type).unwrap_or("<non-utf8>");
    println!("--- seq {} ---", hdr.sequence);
    println!(
        "  ts      : {}.{:09}",
        hdr.timestamp_ns / 1_000_000_000,
        hdr.timestamp_ns % 1_000_000_000
    );
    println!("  cpu     : {}", hdr.cpu_id);
    println!("  origin  : {} ({})", origin_name(hdr.origin), hdr.origin);
    println!("  type    : {:?}", type_str);
    println!("  payload : {} byte(s)", hdr.payload.len());
    if !hdr.payload.is_empty() {
        match msgpack::parse(hdr.payload) {
            Ok((value, rest)) => {
                value.render(4);
                if !rest.is_empty() {
                    println!("    <trailing {} bytes after value>", rest.len());
                }
            }
            Err(e) => {
                println!("    <msgpack decode failed: {e}>");
                print_hex(hdr.payload, 4);
            }
        }
    }
    let _ = Value::Nil; // ensure import isn't dead
}

fn print_hex(b: &[u8], indent: usize) {
    let pre = " ".repeat(indent);
    for chunk in b.chunks(16) {
        let mut hex = String::new();
        for byte in chunk {
            hex.push_str(&format!("{byte:02x} "));
        }
        println!("{pre}{hex}");
    }
}

// ============================================================================
// dump / follow
// ============================================================================

fn cmd_dump() -> ! {
    let mut rings = attach();
    let mut total = 0;
    for r in rings.iter_mut() {
        r.cursor = unsafe { *(r.producer.add(P_TAIL_POS) as *const u64) };
        total += drain_ring(r);
    }
    eprintln!("revstrm: {} event(s) dumped", total);
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

fn cmd_follow(timeout_ms: Option<u64>) -> ! {
    let mut rings = attach();
    for r in rings.iter_mut() {
        r.cursor = unsafe { *(r.producer.add(P_TAIL_POS) as *const u64) };
    }
    let start = std::time::Instant::now();
    let mut last_event = std::time::Instant::now();
    loop {
        let mut got_any = false;
        for r in rings.iter_mut() {
            if drain_ring(r) > 0 {
                got_any = true;
            }
        }
        let _ = std::io::stdout().flush();
        if got_any {
            last_event = std::time::Instant::now();
        }
        if let Some(t) = timeout_ms {
            if last_event.elapsed().as_millis() as u64 >= t {
                eprintln!(
                    "revstrm: idle for {}ms, exiting (started {}ms ago)",
                    t,
                    start.elapsed().as_millis()
                );
                std::process::exit(0);
            }
        }
        sleep_ms(10);
    }
}

// ============================================================================
// main
// ============================================================================

fn usage() -> ! {
    eprintln!(
        "usage: revstrm <emit|dump|follow> [args]\n  \
         revstrm emit <event_type> [payload]\n  \
         revstrm dump\n  \
         revstrm follow [--timeout-ms N]"
    );
    std::process::exit(2);
}

fn main() {
    restore_sigpipe_default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    match args[0].as_str() {
        "emit" => cmd_emit(&args[1..]),
        "dump" => cmd_dump(),
        "follow" => {
            let mut timeout_ms: Option<u64> = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--timeout-ms" => {
                        if i + 1 >= args.len() {
                            usage();
                        }
                        timeout_ms = Some(args[i + 1].parse().unwrap_or_else(|_| usage()));
                        i += 2;
                    }
                    _ => usage(),
                }
            }
            cmd_follow(timeout_ms);
        }
        "-h" | "--help" => usage(),
        _ => usage(),
    }
}

// CString kept available; some emit paths may want it later.
#[allow(dead_code)]
fn _unused() {
    let _ = CString::new("").unwrap();
}
