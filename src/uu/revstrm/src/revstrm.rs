// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (libs) libp msgpack revstrm uucore (vars) regen reattach nonblocking

//! `revstrm` -- print the raw KMES event stream to the terminal.
//!
//! This is a low-level debugging probe: it attaches directly to the KMES
//! per-CPU ring buffers and prints every event it drains. It deliberately
//! knows nothing about eventd. Its second job is to be the *reference
//! consumer* -- it exercises the full PSD-003 §5.1 consumption protocol
//! (per-CPU drain threads, the futex notification wait, generation-change
//! re-attach, lapping/gap detection) so that path is proven before eventd
//! is built.
//!
//! ## Threading model
//!
//! The KMES wake notification is a per-ring futex, and there is no way to
//! block-wait on several rings' futexes from one thread. So the live-follow
//! mode spawns one drain thread per ring -- mirroring eventd's per-CPU model
//! (PSD-008 §2.2). Each thread does the real protocol: drain with the
//! lock-free [`Ring::next`], then block in [`Ring::read_timeout`] when the
//! buffer empties. The timeout is not a poll -- events still wake the thread
//! immediately via the futex; the timeout only lets an idle thread notice a
//! teardown request (generation change) within a bounded interval.
//!
//! The terminal is the one shared resource, so threads serialise on the
//! stdout lock and print directly. There is no fan-in channel: eventd shards
//! per-CPU all the way down, so a single sink would be *less* faithful, not
//! more.
//!
//! `--snapshot` is the exception: it drains whatever is currently buffered
//! with [`Ring::next`] and exits, single-threaded -- there is nothing to
//! wait for, so no futex wait and no per-CPU threads.

use std::fmt::Write as _;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use clap::{Arg, ArgAction, Command};
use uucore::error::{UResult, USimpleError};
use uucore::{format_usage, translate};

use libp_event::{Error as EventError, Origin, Ring};

mod options {
    pub const TYPE: &str = "type";
    pub const ORIGIN: &str = "origin";
    pub const PRETTY: &str = "pretty";
    pub const SNAPSHOT: &str = "snapshot";
}

/// How long an idle drain thread blocks before re-checking the teardown
/// flag. Events arrive via the futex immediately regardless; this only
/// bounds how long a *silent* CPU takes to notice a generation-change
/// re-attach request.
const IDLE_WAIT: Duration = Duration::from_millis(250);

/// Cap on the inline (non-`--pretty`) payload rendering, in bytes of output.
const PAYLOAD_INLINE_CAP: usize = 240;

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;

    // EPIPE -> error return rather than SIGPIPE, so a downstream `head`
    // closing the pipe shuts us down cleanly instead of killing us.
    #[cfg(unix)]
    let _ = uucore::signals::disable_pipe_errors();

    let printer = Arc::new(Printer {
        filter: Filter::from_matches(&matches)?,
        pretty: matches.get_flag(options::PRETTY),
    });

    if matches.get_flag(options::SNAPSHOT) {
        run_snapshot(&printer)
    } else {
        run_follow(&printer)
    }
}

/// Live follow: one blocking drain thread per ring, re-attaching on a
/// generation change. Runs until the process is signalled (e.g. Ctrl-C) or
/// stdout closes.
fn run_follow(printer: &Arc<Printer>) -> UResult<()> {
    loop {
        let rings = attach()?;

        // `regen` is raised by whichever thread sees a generation change;
        // all threads then exit and we re-attach the resized buffers.
        let regen = Arc::new(AtomicBool::new(false));

        let handles: Vec<_> = rings
            .into_iter()
            .map(|ring| {
                let printer = Arc::clone(printer);
                let regen = Arc::clone(&regen);
                thread::spawn(move || drain_thread(ring, printer, regen))
            })
            .collect();

        for handle in handles {
            // A panicking drain thread shouldn't take the others' output
            // with it; just note it and carry on.
            if handle.join().is_err() {
                eprintln!("{}: a drain thread panicked", uucore::util_name());
            }
        }

        if regen.load(Ordering::Acquire) {
            // Buffers were resized; re-attach and respawn.
            continue;
        }
        // All threads exited without a re-attach request -- a fatal ring
        // error on every CPU. Nothing left to drain.
        return Ok(());
    }
}

/// One per-CPU drain thread running the full PSD-003 §5.1 protocol.
fn drain_thread(mut ring: Ring, printer: Arc<Printer>, regen: Arc<AtomicBool>) {
    let mut reported_lost: u64 = 0;
    loop {
        // Phase 1: lock-free drain of everything currently available.
        loop {
            match ring.next() {
                Ok(Some(ev)) => printer.emit(
                    ev.cpu_id,
                    ev.sequence,
                    ev.timestamp_ns,
                    ev.origin,
                    ev.event_type,
                    ev.payload,
                ),
                Ok(None) => break,
                Err(EventError::GenerationChanged) => {
                    regen.store(true, Ordering::Release);
                    return;
                }
                Err(e) => {
                    printer.note_ring_error(ring.cpu_id(), &e);
                    return;
                }
            }
        }
        printer.report_lost(ring.cpu_id(), ring.lost_events(), &mut reported_lost);

        // Phase 2: block in the futex notification wait until an event
        // arrives or the idle interval elapses (so we can notice a teardown).
        match ring.read_timeout(IDLE_WAIT) {
            Ok(Some(ev)) => printer.emit(
                ev.cpu_id,
                ev.sequence,
                ev.timestamp_ns,
                ev.origin,
                &ev.event_type,
                &ev.payload,
            ),
            Ok(None) => {} // idle timeout; loop and drain again
            Err(EventError::Interrupted) => {} // stray signal; resume
            Err(EventError::GenerationChanged) => {
                regen.store(true, Ordering::Release);
                return;
            }
            Err(e) => {
                printer.note_ring_error(ring.cpu_id(), &e);
                return;
            }
        }
    }
}

/// Snapshot: drain whatever is buffered across all rings right now, in
/// CPU order, then exit. Single-threaded -- no waiting.
fn run_snapshot(printer: &Arc<Printer>) -> UResult<()> {
    let mut rings = attach()?;
    for ring in &mut rings {
        let mut reported_lost: u64 = 0;
        loop {
            match ring.next() {
                Ok(Some(ev)) => printer.emit(
                    ev.cpu_id,
                    ev.sequence,
                    ev.timestamp_ns,
                    ev.origin,
                    ev.event_type,
                    ev.payload,
                ),
                Ok(None) => break,
                // A resize mid-snapshot just ends this ring's drain; the
                // snapshot is best-effort point-in-time anyway.
                Err(EventError::GenerationChanged) => break,
                Err(e) => {
                    printer.note_ring_error(ring.cpu_id(), &e);
                    break;
                }
            }
        }
        printer.report_lost(ring.cpu_id(), ring.lost_events(), &mut reported_lost);
    }
    Ok(())
}

/// Attach to all per-CPU rings, mapping a privilege failure to a clear hint.
fn attach() -> UResult<Vec<Ring>> {
    Ring::attach_all().map_err(|e| {
        USimpleError::new(1, format!("{}: {e}", translate!("revstrm-error-attach")))
    })
}

// ---------------------------------------------------------------------------
// Filtering + formatting (the printer side; orthogonal to the protocol).
// ---------------------------------------------------------------------------

/// Shared, read-only formatter handed to every drain thread.
struct Printer {
    filter: Filter,
    pretty: bool,
}

impl Printer {
    /// Format and print one event if it passes the filter. Holds the stdout
    /// lock for the whole write so a line is never interleaved with another
    /// CPU's. A write failure (e.g. downstream pipe closed) ends the process.
    fn emit(&self, cpu: u16, seq: u64, ts_ns: u64, origin: u8, ev_type: &[u8], payload: &[u8]) {
        if !self.filter.matches(origin, ev_type) {
            return;
        }

        let ty = String::from_utf8_lossy(ev_type);
        let stdout = io::stdout();
        let mut out = stdout.lock();

        let res = (|| {
            write!(
                out,
                "{}  cpu{cpu:<2} #{seq:<8} {:>4}  {ty}",
                fmt_time(ts_ns),
                origin_label(origin),
            )?;
            if self.pretty {
                writeln!(out)?;
                render_payload_pretty(&mut out, payload, 4)
            } else {
                writeln!(out, "  {}", render_payload_inline(payload))
            }
        })();

        if res.is_err() {
            // Broken pipe / closed terminal: nothing left to print to.
            std::process::exit(0);
        }
    }

    /// Emit a visible marker when a ring lapped and events were lost. Never
    /// silent -- a dropped event the user can't see is the worst outcome for
    /// a debugger.
    fn report_lost(&self, cpu: u16, total_lost: u64, reported: &mut u64) {
        if total_lost > *reported {
            let n = total_lost - *reported;
            *reported = total_lost;
            let stdout = io::stdout();
            let mut out = stdout.lock();
            let _ = writeln!(out, "--- cpu{cpu}: lost {n} event(s) (ring lapped) ---");
        }
    }

    fn note_ring_error(&self, cpu: u16, e: &EventError) {
        eprintln!("{}: cpu{cpu}: {e}", uucore::util_name());
    }
}

/// Event filter. An empty list for a dimension matches everything.
#[derive(Clone)]
struct Filter {
    /// Globs matched against the event-type string (any-match = OR).
    types: Vec<glob::Pattern>,
    /// Permitted raw `origin_class` bytes.
    origins: Vec<u8>,
}

impl Filter {
    fn from_matches(matches: &clap::ArgMatches) -> UResult<Self> {
        let types = matches
            .get_many::<String>(options::TYPE)
            .into_iter()
            .flatten()
            .map(|p| {
                glob::Pattern::new(p).map_err(|e| {
                    USimpleError::new(
                        1,
                        format!("{}: {p:?}: {e}", translate!("revstrm-error-bad-pattern")),
                    )
                })
            })
            .collect::<UResult<Vec<_>>>()?;

        let origins = matches
            .get_many::<String>(options::ORIGIN)
            .into_iter()
            .flatten()
            .map(|o| parse_origin(o))
            .collect::<UResult<Vec<_>>>()?;

        Ok(Self { types, origins })
    }

    fn matches(&self, origin: u8, ev_type: &[u8]) -> bool {
        if !self.origins.is_empty() && !self.origins.contains(&origin) {
            return false;
        }
        if !self.types.is_empty() {
            let s = String::from_utf8_lossy(ev_type);
            if !self.types.iter().any(|p| p.matches(&s)) {
                return false;
            }
        }
        true
    }
}

/// Map an `--origin` argument to its raw `origin_class` byte. Accepts the
/// subsystem names and a couple of obvious aliases, case-insensitively.
fn parse_origin(s: &str) -> UResult<u8> {
    let o = match s.to_ascii_lowercase().as_str() {
        "userspace" | "user" | "usr" => Origin::Userspace,
        "kmes" => Origin::Kmes,
        "kacs" => Origin::Kacs,
        "lcs" => Origin::Lcs,
        other => {
            return Err(USimpleError::new(
                1,
                format!("{}: {other:?}", translate!("revstrm-error-bad-origin")),
            ));
        }
    };
    Ok(o.as_raw())
}

/// Short, fixed-width-friendly label for an origin class.
fn origin_label(origin: u8) -> String {
    match Origin::from_raw(origin) {
        Origin::Userspace => "USR".to_string(),
        Origin::Kmes => "KMES".to_string(),
        Origin::Kacs => "KACS".to_string(),
        Origin::Lcs => "LCS".to_string(),
        Origin::Other(o) => format!("c{o}"),
    }
}

/// Render `timestamp_ns` (Unix nanoseconds) as a UTC time-of-day with
/// microsecond precision: `HH:MM:SS.uuuuuu`. Day/date is dropped -- a live
/// tail cares about wall-clock time of day, not the calendar.
fn fmt_time(ts_ns: u64) -> String {
    let secs = ts_ns / 1_000_000_000;
    let micros = (ts_ns % 1_000_000_000) / 1_000;
    let tod = secs % 86_400;
    let (h, m, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    format!("{h:02}:{m:02}:{s:02}.{micros:06}")
}

// --- msgpack payload rendering --------------------------------------------

/// One-line, compact rendering of a msgpack payload, truncated to
/// [`PAYLOAD_INLINE_CAP`]. Falls back to a hex preview if it won't decode.
fn render_payload_inline(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "(empty)".to_string();
    }
    match msgpack::parse(payload) {
        Ok((value, rest)) => {
            let mut s = String::new();
            compact(&value, &mut s);
            if !rest.is_empty() {
                let _ = write!(s, " (+{} trailing byte(s))", rest.len());
            }
            truncate(s, PAYLOAD_INLINE_CAP)
        }
        Err(e) => format!(
            "<{} byte(s), undecodable: {e}> {}",
            payload.len(),
            hex_preview(payload),
        ),
    }
}

/// Compact recursive rendering of a decoded value into `buf`. Arrays and
/// maps stay on one line; the caller truncates the result.
fn compact(value: &msgpack::Value, buf: &mut String) {
    use msgpack::Value::*;
    match value {
        Nil => buf.push_str("nil"),
        Bool(b) => buf.push_str(if *b { "true" } else { "false" }),
        UInt(u) => {
            let _ = write!(buf, "{u}");
        }
        Int(i) => {
            let _ = write!(buf, "{i}");
        }
        F32(x) => {
            let _ = write!(buf, "{x}");
        }
        F64(x) => {
            let _ = write!(buf, "{x}");
        }
        Str(s) => {
            let _ = write!(buf, "{s:?}");
        }
        Bin(b) => buf.push_str(&hex_preview(b)),
        Ext { ty, data } => {
            let _ = write!(buf, "ext({ty}, {} byte(s))", data.len());
        }
        Array(items) => {
            buf.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                compact(item, buf);
            }
            buf.push(']');
        }
        Map(items) => {
            buf.push('{');
            for (i, (k, v)) in items.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                compact(k, buf);
                buf.push_str(": ");
                match k {
                    Str(key) => {
                        match render_sid_field(key, v).or_else(|| render_access_field(key, v)) {
                            Some(rendered) => buf.push_str(&rendered),
                            None => compact(v, buf),
                        }
                    }
                    _ => compact(v, buf),
                }
            }
            buf.push('}');
        }
    }
}

/// If `key` names a SID field — suffix `sid` (one SID) or `sids` (an array of
/// SIDs) — render its msgpack-`bin` value(s) as canonical string SID(s) like
/// `S-1-5-18`. Returns `None` if it isn't a SID field, or the value isn't the
/// shape a SID field should hold, so the caller renders it normally. A `bin`
/// that doesn't parse as a SID falls back to a hex preview rather than hiding.
fn render_sid_field(key: &str, value: &msgpack::Value) -> Option<String> {
    use msgpack::Value::{Array, Bin};
    let key = key.to_ascii_lowercase();
    if key.ends_with("sids") {
        let Array(items) = value else { return None };
        let parts: Vec<String> = items
            .iter()
            .map(|v| match v {
                Bin(b) => sid_to_string(b).unwrap_or_else(|| hex_preview(b)),
                other => {
                    let mut s = String::new();
                    compact(other, &mut s);
                    s
                }
            })
            .collect();
        Some(format!("[{}]", parts.join(", ")))
    } else if key.ends_with("sid") {
        match value {
            Bin(b) => Some(sid_to_string(b).unwrap_or_else(|| hex_preview(b))),
            _ => None,
        }
    } else {
        None
    }
}

/// The access-mask bits, low to high. The low 16 bits are object-specific;
/// every KACS `access-audit` event is filesystem access, so they are named as
/// `FILE_*` rights. Names match `kacs-core::access_mask`.
const ACCESS_FLAGS: &[(u32, &str)] = &[
    (0x0000_0001, "FILE_READ_DATA"),
    (0x0000_0002, "FILE_WRITE_DATA"),
    (0x0000_0004, "FILE_APPEND_DATA"),
    (0x0000_0008, "FILE_READ_EA"),
    (0x0000_0010, "FILE_WRITE_EA"),
    (0x0000_0020, "FILE_EXECUTE"),
    (0x0000_0040, "FILE_DELETE_CHILD"),
    (0x0000_0080, "FILE_READ_ATTRIBUTES"),
    (0x0000_0100, "FILE_WRITE_ATTRIBUTES"),
    (0x0001_0000, "DELETE"),
    (0x0002_0000, "READ_CONTROL"),
    (0x0004_0000, "WRITE_DAC"),
    (0x0008_0000, "WRITE_OWNER"),
    (0x0010_0000, "SYNCHRONIZE"),
    (0x0100_0000, "ACCESS_SYSTEM_SECURITY"),
    (0x0200_0000, "MAXIMUM_ALLOWED"),
    (0x1000_0000, "GENERIC_ALL"),
    (0x2000_0000, "GENERIC_EXECUTE"),
    (0x4000_0000, "GENERIC_WRITE"),
    (0x8000_0000, "GENERIC_READ"),
];

/// If `key` names an access mask — suffix `access` — decode its `uint` value
/// into `|`-joined right names. Returns `None` if it isn't an access field or
/// the value isn't a `u32`-sized uint, so the caller renders it normally.
fn render_access_field(key: &str, value: &msgpack::Value) -> Option<String> {
    if !key.to_ascii_lowercase().ends_with("access") {
        return None;
    }
    match value {
        msgpack::Value::UInt(m) => u32::try_from(*m).ok().map(decode_access_mask),
        _ => None,
    }
}

/// Decode an access mask into `|`-joined right names. Any bits without a known
/// name are appended as a single `0x…` remainder, so nothing is hidden.
fn decode_access_mask(mask: u32) -> String {
    if mask == 0 {
        return "0 (none)".to_string();
    }
    let mut parts: Vec<&str> = Vec::new();
    let mut remaining = mask;
    for (bit, name) in ACCESS_FLAGS {
        if remaining & bit != 0 {
            parts.push(name);
            remaining &= !bit;
        }
    }
    let mut s = parts.join("|");
    if remaining != 0 {
        if !s.is_empty() {
            s.push('|');
        }
        let _ = write!(s, "0x{remaining:08x}");
    }
    s
}

/// Parse a binary SID (MS-DTYP self-relative layout: revision, sub-authority
/// count, 6-byte big-endian identifier authority, then little-endian
/// sub-authorities) into its canonical string form
/// `S-<rev>-<authority>[-<subauth>...]`. Returns `None` if the length doesn't
/// match the declared sub-authority count. The authority renders as decimal,
/// or `0x`-hex if it exceeds 32 bits (per the MS-DTYP convention).
fn sid_to_string(b: &[u8]) -> Option<String> {
    if b.len() < 8 {
        return None;
    }
    let revision = b[0];
    let sub_count = b[1] as usize;
    if b.len() != 8 + 4 * sub_count {
        return None;
    }
    let authority = b[2..8].iter().fold(0u64, |acc, &x| (acc << 8) | u64::from(x));
    let mut s = String::from("S-");
    let _ = write!(s, "{revision}-");
    if authority < 0x1_0000_0000 {
        let _ = write!(s, "{authority}");
    } else {
        let _ = write!(s, "0x{authority:012x}");
    }
    for i in 0..sub_count {
        let off = 8 + 4 * i;
        let sub = u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]);
        let _ = write!(s, "-{sub}");
    }
    Some(s)
}

/// Multi-line pretty rendering of a msgpack payload, written to `out` at the
/// given indent. Mirrors `msgpack::Value::render` but targets our writer --
/// `Value::render` uses `println!`, which would deadlock against the stdout
/// lock the caller already holds.
fn render_payload_pretty(out: &mut impl Write, payload: &[u8], indent: usize) -> io::Result<()> {
    let pad = " ".repeat(indent);
    if payload.is_empty() {
        return writeln!(out, "{pad}(empty)");
    }
    match msgpack::parse(payload) {
        Ok((value, rest)) => {
            render_value(out, &value, indent)?;
            if !rest.is_empty() {
                writeln!(out, "{pad}(+{} trailing byte(s))", rest.len())?;
            }
            Ok(())
        }
        Err(e) => writeln!(
            out,
            "{pad}<{} byte(s), undecodable: {e}> {}",
            payload.len(),
            hex_preview(payload),
        ),
    }
}

fn render_value(out: &mut impl Write, value: &msgpack::Value, indent: usize) -> io::Result<()> {
    use msgpack::Value::{Array, Map};
    match value {
        Map(items) => render_map(out, items, indent),
        Array(items) => render_array(out, items, indent),
        scalar => {
            let mut s = String::new();
            compact(scalar, &mut s);
            writeln!(out, "{}{s}", " ".repeat(indent))
        }
    }
}

/// Pretty-print a map as an aligned `key   value` block: bare (unquoted) keys,
/// scalar values rendered cleanly via [`compact`] and column-aligned, nested
/// maps/arrays expanded beneath their key. SID-suffixed keys render string
/// SIDs. No `map(N)`/type-name noise.
fn render_map(
    out: &mut impl Write,
    items: &[(msgpack::Value, msgpack::Value)],
    indent: usize,
) -> io::Result<()> {
    let pad = " ".repeat(indent);
    let width = items.iter().map(|(k, _)| key_label(k).len()).max().unwrap_or(0);
    for (k, v) in items {
        let key = key_label(k);
        // SID-/access-suffixed keys render inline regardless of value shape.
        if let msgpack::Value::Str(ks) = k {
            if let Some(rendered) = render_sid_field(ks, v).or_else(|| render_access_field(ks, v)) {
                writeln!(out, "{pad}{key:<width$}  {rendered}")?;
                continue;
            }
        }
        if expands(v) {
            writeln!(out, "{pad}{key}:")?;
            render_value(out, v, indent + 2)?;
        } else {
            let mut s = String::new();
            compact(v, &mut s);
            writeln!(out, "{pad}{key:<width$}  {s}")?;
        }
    }
    Ok(())
}

/// Pretty-print an array of containers, one indexed element per block. (Arrays
/// of scalars never reach here -- the caller renders them inline via `compact`.)
fn render_array(out: &mut impl Write, items: &[msgpack::Value], indent: usize) -> io::Result<()> {
    let pad = " ".repeat(indent);
    if items.is_empty() {
        return writeln!(out, "{pad}[]");
    }
    for (i, v) in items.iter().enumerate() {
        if expands(v) {
            writeln!(out, "{pad}[{i}]:")?;
            render_value(out, v, indent + 2)?;
        } else {
            let mut s = String::new();
            compact(v, &mut s);
            writeln!(out, "{pad}[{i}]  {s}")?;
        }
    }
    Ok(())
}

/// A value gets its own indented block (rather than an inline render) when it
/// is a non-empty map, or an array that holds a map/array. Scalars, empty
/// maps, and scalar-only arrays render inline.
fn expands(v: &msgpack::Value) -> bool {
    use msgpack::Value::{Array, Map};
    match v {
        Map(items) => !items.is_empty(),
        Array(items) => items.iter().any(|e| matches!(e, Map(_) | Array(_))),
        _ => false,
    }
}

/// A map key as bare text: a string key without quotes; anything else via
/// [`compact`].
fn key_label(k: &msgpack::Value) -> String {
    match k {
        msgpack::Value::Str(s) => s.clone(),
        other => {
            let mut s = String::new();
            compact(other, &mut s);
            s
        }
    }
}

/// `0x`-prefixed hex preview of up to 32 bytes, with an ellipsis if longer.
fn hex_preview(bytes: &[u8]) -> String {
    const CAP: usize = 32;
    let mut s = String::from("0x");
    for b in bytes.iter().take(CAP) {
        let _ = write!(s, "{b:02x}");
    }
    if bytes.len() > CAP {
        s.push('…');
    }
    s
}

/// Truncate `s` to at most `cap` chars, appending an ellipsis if it was cut.
fn truncate(mut s: String, cap: usize) -> String {
    if s.chars().count() > cap {
        let end = s.char_indices().nth(cap).map_or(s.len(), |(i, _)| i);
        s.truncate(end);
        s.push('…');
    }
    s
}

// ---------------------------------------------------------------------------

pub fn uu_app() -> Command {
    let cmd = Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .about(translate!("revstrm-about"))
        .override_usage(format_usage(&translate!("revstrm-usage")))
        .after_help(translate!("revstrm-after-help"))
        .infer_long_args(true);
    uucore::clap_localization::configure_localized_command(cmd)
        .arg(
            Arg::new(options::TYPE)
                .long("type")
                .short('t')
                .value_name("GLOB")
                .action(ArgAction::Append)
                .help(translate!("revstrm-help-type")),
        )
        .arg(
            Arg::new(options::ORIGIN)
                .long("origin")
                .short('o')
                .value_name("CLASS")
                .action(ArgAction::Append)
                .help(translate!("revstrm-help-origin")),
        )
        .arg(
            Arg::new(options::PRETTY)
                .long("pretty")
                .short('p')
                .action(ArgAction::SetTrue)
                .help(translate!("revstrm-help-pretty")),
        )
        .arg(
            Arg::new(options::SNAPSHOT)
                .long("snapshot")
                .short('s')
                .action(ArgAction::SetTrue)
                .help(translate!("revstrm-help-snapshot")),
        )
}

#[cfg(test)]
mod tests {
    use super::{compact, render_sid_field, sid_to_string};
    use msgpack::Value;

    fn sid(authority: u64, subs: &[u32]) -> Vec<u8> {
        let mut b = vec![1u8, subs.len() as u8];
        b.extend_from_slice(&authority.to_be_bytes()[2..]); // 6-byte big-endian authority
        for s in subs {
            b.extend_from_slice(&s.to_le_bytes());
        }
        b
    }

    #[test]
    fn parses_canonical_sids() {
        assert_eq!(sid_to_string(&sid(5, &[18])).as_deref(), Some("S-1-5-18"));
        assert_eq!(sid_to_string(&sid(5, &[32, 544])).as_deref(), Some("S-1-5-32-544"));
        assert_eq!(sid_to_string(&sid(1, &[0])).as_deref(), Some("S-1-1-0"));
        assert_eq!(sid_to_string(&sid(11, &[])).as_deref(), Some("S-1-11"));
        // Length disagrees with the sub-authority count -> not a SID.
        assert_eq!(sid_to_string(&[1, 2, 0, 0, 0, 0, 0, 5]), None);
        assert_eq!(sid_to_string(&[]), None);
    }

    #[test]
    fn sid_suffix_fields_render_as_strings() {
        let bin = |b: Vec<u8>| Value::Bin(b);
        let map = Value::Map(vec![
            (Value::Str("user_sid".into()), bin(sid(5, &[18]))),
            (
                Value::Str("group_sids".into()),
                Value::Array(vec![bin(sid(1, &[0])), bin(sid(5, &[32, 544]))]),
            ),
            (Value::Str("policy_sid".into()), bin(sid(5, &[11]))),
            (Value::Str("count".into()), Value::UInt(3)),
        ]);
        let mut out = String::new();
        compact(&map, &mut out);
        assert!(out.contains("\"user_sid\": S-1-5-18"), "{out}");
        assert!(
            out.contains("\"group_sids\": [S-1-1-0, S-1-5-32-544]"),
            "{out}"
        );
        assert!(out.contains("\"policy_sid\": S-1-5-11"), "{out}");
        assert!(out.contains("\"count\": 3"), "{out}");
    }

    #[test]
    fn non_sid_keys_and_bad_shapes_fall_through() {
        // A non-SID key is not rewritten.
        assert_eq!(render_sid_field("count", &Value::UInt(1)), None);
        // A `sid` key whose value is not binary falls through to normal render.
        assert_eq!(render_sid_field("user_sid", &Value::UInt(1)), None);
        // A `sid`-suffixed bin that isn't a valid SID falls back to hex.
        let r = render_sid_field("weird_sid", &Value::Bin(vec![0xde, 0xad])).unwrap();
        assert!(r.starts_with("0xdead"), "{r}");
    }

    #[test]
    fn decodes_access_masks() {
        use super::decode_access_mask;
        assert_eq!(decode_access_mask(0x80), "FILE_READ_ATTRIBUTES");
        assert_eq!(
            decode_access_mask(0x0106_0080),
            "FILE_READ_ATTRIBUTES|READ_CONTROL|WRITE_DAC|ACCESS_SYSTEM_SECURITY"
        );
        assert_eq!(decode_access_mask(0x2000_0000), "GENERIC_EXECUTE");
        assert_eq!(decode_access_mask(0), "0 (none)");
        // An undecodable bit is surfaced as a hex remainder, not hidden.
        assert_eq!(decode_access_mask(0x0000_0200), "0x00000200");
        assert_eq!(
            decode_access_mask(0x0000_0280),
            "FILE_READ_ATTRIBUTES|0x00000200"
        );
    }

    #[test]
    fn access_suffix_fields_decode() {
        use super::render_access_field;
        assert_eq!(
            render_access_field("granted_access", &Value::UInt(0x0106_0080)).as_deref(),
            Some("FILE_READ_ATTRIBUTES|READ_CONTROL|WRITE_DAC|ACCESS_SYSTEM_SECURITY")
        );
        // Non-access key, and a wrong-typed value, both fall through.
        assert_eq!(render_access_field("count", &Value::UInt(5)), None);
        assert_eq!(
            render_access_field("granted_access", &Value::Str("x".into())),
            None
        );
    }
}
