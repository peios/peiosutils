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
//! lock-free [`EventReader::next`], then block in [`EventReader::wait`] when
//! the buffer empties. The wait is not a poll -- events still wake the thread
//! immediately via the futex; the timeout only lets an idle thread notice a
//! teardown request within a bounded interval. The [`EventReader`] absorbs the
//! lapping/gap accounting and the generation-change re-attach internally, so
//! the drain loop here just alternates `next` / `wait`.
//!
//! The terminal is the one shared resource, so threads serialise on the
//! stdout lock and print directly. There is no fan-in channel: eventd shards
//! per-CPU all the way down, so a single sink would be *less* faithful, not
//! more.
//!
//! `--snapshot` is the exception: it drains whatever is currently buffered
//! with [`EventReader::next`] and exits, single-threaded -- there is nothing
//! to wait for, so no futex wait and no per-CPU threads.

use std::fmt::Write as _;
use std::io::{self, Write};
use std::sync::Arc;
use std::thread;

use clap::{Arg, ArgAction, Command};
use uucore::error::{UResult, USimpleError};
use uucore::{format_usage, translate};

use peios::event::{Event, EventReader, OriginClass};
use peios::msgpack::{Reader, Type};

mod options {
    pub const TYPE: &str = "type";
    pub const ORIGIN: &str = "origin";
    pub const PRETTY: &str = "pretty";
    pub const SNAPSHOT: &str = "snapshot";
}

/// How long an idle drain thread blocks in the futex wait before looping back
/// to re-drain. Events arrive via the futex immediately regardless; this only
/// bounds how long a *silent* CPU sleeps between wake checks.
const IDLE_WAIT_MS: i32 = 250;

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

/// An [`EventReader`] moved across a thread boundary.
///
/// `EventReader` is `!Send` because it holds a raw pointer to its kernel-side
/// reader, but a reader is only ever owned and driven by a single thread (each
/// drain thread gets its own), so handing exclusive ownership to that thread at
/// spawn is sound. This newtype attests that.
struct SendReader(EventReader);

// SAFETY: the wrapped reader is moved by value into exactly one drain thread,
// which then owns it exclusively for its whole life; it is never shared or
// touched from two threads at once.
unsafe impl Send for SendReader {}

/// Emit one event through the printer, copying the borrowed header/payload out
/// first so the reader is free to advance.
fn emit_event(printer: &Printer, cpu: u16, ev: &Event<'_>) {
    printer.emit(
        cpu,
        ev.sequence,
        ev.timestamp,
        origin_raw(ev.origin_class),
        ev.event_type.as_bytes(),
        ev.payload,
    );
}

/// Live follow: one blocking drain thread per CPU ring. The [`EventReader`]
/// absorbs generation-change re-attach internally, so each thread simply
/// alternates a lock-free drain with a futex wait, forever. Runs until the
/// process is signalled (e.g. Ctrl-C) or stdout closes.
fn run_follow(printer: &Arc<Printer>) -> UResult<()> {
    let readers = attach()?;

    let handles: Vec<_> = readers
        .into_iter()
        .map(|(cpu, reader)| {
            let printer = Arc::clone(printer);
            thread::spawn(move || drain_thread(cpu, reader, printer))
        })
        .collect();

    for handle in handles {
        // A panicking drain thread shouldn't take the others' output with it;
        // just note it and carry on.
        if handle.join().is_err() {
            eprintln!("{}: a drain thread panicked", uucore::util_name());
        }
    }
    // All threads exited -- a fatal ring error on every CPU. Nothing left.
    Ok(())
}

/// One per-CPU drain thread running the full PSD-003 §5.1 protocol. The
/// [`EventReader`] hides the generation-change re-attach, so this loop only
/// has to alternate drain (`next`) and futex wait (`wait`).
fn drain_thread(cpu: u16, reader: SendReader, printer: Arc<Printer>) {
    let SendReader(mut reader) = reader;
    let mut reported_lost: u64 = 0;
    loop {
        // Phase 1: lock-free drain of everything currently available.
        loop {
            match reader.next() {
                Ok(Some(ev)) => emit_event(&printer, cpu, &ev),
                Ok(None) => break,
                Err(e) => {
                    printer.note_ring_error(cpu, &e);
                    return;
                }
            }
        }
        printer.report_lost(cpu, reader.lost(), &mut reported_lost);

        // Phase 2: block in the futex notification wait until an event arrives
        // or the idle interval elapses, then loop back to drain. `wait`
        // returns `false` on a timeout or a stray wake -- either way we just
        // re-drain.
        if let Err(e) = reader.wait(IDLE_WAIT_MS) {
            printer.note_ring_error(cpu, &e);
            return;
        }
    }
}

/// Snapshot: drain whatever is buffered across all rings right now, in
/// CPU order, then exit. Single-threaded -- no waiting.
fn run_snapshot(printer: &Arc<Printer>) -> UResult<()> {
    let readers = attach()?;
    for (cpu, SendReader(mut reader)) in readers {
        let mut reported_lost: u64 = 0;
        loop {
            match reader.next() {
                Ok(Some(ev)) => emit_event(printer, cpu, &ev),
                Ok(None) => break,
                Err(e) => {
                    printer.note_ring_error(cpu, &e);
                    break;
                }
            }
        }
        printer.report_lost(cpu, reader.lost(), &mut reported_lost);
    }
    Ok(())
}

/// Attach to every per-CPU ring, mapping a privilege failure to a clear hint.
///
/// There is no `attach_all`; instead enumerate CPUs by opening rings counting
/// up from `0` until an `EINVAL` reports there are no more. The first CPU
/// failing for any *other* reason (e.g. a missing privilege) is a hard error.
fn attach() -> UResult<Vec<(u16, SendReader)>> {
    let mut readers = Vec::new();
    let mut cpu: u32 = 0;
    loop {
        match EventReader::open(cpu) {
            Ok(reader) => {
                readers.push((cpu as u16, SendReader(reader)));
                cpu += 1;
            }
            // EINVAL past the last CPU just ends enumeration.
            Err(e) if e.raw_os_error() == Some(libc::EINVAL) && cpu > 0 => break,
            Err(e) => {
                return Err(USimpleError::new(
                    1,
                    format!("{}: {e}", translate!("revstrm-error-attach")),
                ));
            }
        }
    }
    Ok(readers)
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

    fn note_ring_error(&self, cpu: u16, e: &peios::Error) {
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

/// The raw `origin_class` byte for an [`OriginClass`]. (`OriginClass` does not
/// expose its discriminant, so the mapping is kept here -- it must stay in step
/// with the kernel's `origin_class` numbering, which `OriginClass::from_raw`
/// also encodes.)
fn origin_raw(o: OriginClass) -> u8 {
    match o {
        OriginClass::Userspace => 0,
        OriginClass::Kmes => 1,
        OriginClass::Kacs => 2,
        OriginClass::Lcs => 3,
        OriginClass::Other(o) => o,
    }
}

/// Map a raw `origin_class` byte back to an [`OriginClass`], mirroring the
/// kernel numbering used by [`origin_raw`].
fn origin_from_raw(v: u8) -> OriginClass {
    match v {
        0 => OriginClass::Userspace,
        1 => OriginClass::Kmes,
        2 => OriginClass::Kacs,
        3 => OriginClass::Lcs,
        other => OriginClass::Other(other),
    }
}

/// Map an `--origin` argument to its raw `origin_class` byte. Accepts the
/// subsystem names and a couple of obvious aliases, case-insensitively.
fn parse_origin(s: &str) -> UResult<u8> {
    let o = match s.to_ascii_lowercase().as_str() {
        "userspace" | "user" | "usr" => OriginClass::Userspace,
        "kmes" => OriginClass::Kmes,
        "kacs" => OriginClass::Kacs,
        "lcs" => OriginClass::Lcs,
        other => {
            return Err(USimpleError::new(
                1,
                format!("{}: {other:?}", translate!("revstrm-error-bad-origin")),
            ));
        }
    };
    Ok(origin_raw(o))
}

/// Short, fixed-width-friendly label for an origin class.
fn origin_label(origin: u8) -> String {
    match origin_from_raw(origin) {
        OriginClass::Userspace => "USR".to_string(),
        OriginClass::Kmes => "KMES".to_string(),
        OriginClass::Kacs => "KACS".to_string(),
        OriginClass::Lcs => "LCS".to_string(),
        OriginClass::Other(o) => format!("c{o}"),
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

// --- streaming msgpack rendering ------------------------------------------
//
// The codec is decode-only and streaming: a `Reader` is a forward cursor with
// `peek()` to look at the next value's type and `read_*` to consume it. The
// payload is a single pre-order value tree, so rendering is a recursive descent
// that mirrors the old tree walk -- read a node's type via `peek`, dispatch to
// the matching `read_*` (recursing into `read_array` / `read_map` children).
//
// A malformed payload surfaces mid-stream as a `read_*` error rather than up
// front, so the descent returns `Result` and the entry points fall back to a
// hex preview on any error.

/// One-line, compact rendering of a msgpack payload, truncated to
/// [`PAYLOAD_INLINE_CAP`]. Falls back to a hex preview if it won't decode.
fn render_payload_inline(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "(empty)".to_string();
    }
    let mut reader = Reader::new(payload);
    let mut s = String::new();
    match compact(&mut reader, &mut s) {
        Ok(()) => {
            let rest = reader.remaining();
            if rest != 0 {
                let _ = write!(s, " (+{rest} trailing byte(s))");
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

/// Compact recursive rendering of the value at `reader`'s cursor into `buf`,
/// consuming exactly that one value. Arrays and maps stay on one line; the
/// caller truncates the result.
fn compact(reader: &mut Reader, buf: &mut String) -> peios::Result<()> {
    match reader.peek() {
        None => {
            // No valid lead byte where a value was expected: force the error.
            reader.read_nil()?;
        }
        Some(Type::Nil) => {
            reader.read_nil()?;
            buf.push_str("nil");
        }
        Some(Type::Bool) => {
            let b = reader.read_bool()?;
            buf.push_str(if b { "true" } else { "false" });
        }
        Some(Type::Int) => {
            // The codec reports all integers as `Int`; render unsigned values
            // that overflow `i64` faithfully by trying the unsigned read first.
            compact_int(reader, buf)?;
        }
        Some(Type::Float) => {
            let x = reader.read_float()?;
            let _ = write!(buf, "{x}");
        }
        Some(Type::Str) => {
            let s = reader.read_str()?;
            let _ = write!(buf, "{s:?}");
        }
        Some(Type::Bin) => {
            let b = reader.read_bin()?;
            buf.push_str(&hex_preview(b));
        }
        Some(Type::Ext) => {
            let (ty, data) = reader.read_ext()?;
            let _ = write!(buf, "ext({ty}, {} byte(s))", data.len());
        }
        Some(Type::Array) => {
            let n = reader.read_array()?;
            buf.push('[');
            for i in 0..n {
                if i > 0 {
                    buf.push_str(", ");
                }
                compact(reader, buf)?;
            }
            buf.push(']');
        }
        Some(Type::Map) => {
            let n = reader.read_map()?;
            buf.push('{');
            for i in 0..n {
                if i > 0 {
                    buf.push_str(", ");
                }
                compact_pair(reader, buf)?;
            }
            buf.push('}');
        }
    }
    Ok(())
}

/// Render one map key/value pair compactly as `key: value`, applying the
/// key-driven SID/access special rendering to the value when the key is a
/// string with a recognised suffix.
fn compact_pair(reader: &mut Reader, buf: &mut String) -> peios::Result<()> {
    if reader.peek() == Some(Type::Str) {
        let key = reader.read_str()?.to_string();
        let _ = write!(buf, "{key:?}: ");
        match render_special_value(reader, &key)? {
            Some(rendered) => buf.push_str(&rendered),
            None => compact(reader, buf)?,
        }
    } else {
        // Non-string key: render key then value plainly.
        compact(reader, buf)?;
        buf.push_str(": ");
        compact(reader, buf)?;
    }
    Ok(())
}

/// Render an integer node, preferring the unsigned reading so values above
/// `i64::MAX` print correctly (the codec collapses signed/unsigned into one
/// `Int` type, leaving the choice to the reader).
fn compact_int(reader: &mut Reader, buf: &mut String) -> peios::Result<()> {
    // A non-negative msgpack int reads fine as unsigned; a negative one does
    // not, so fall back to the signed read. `read_uint` leaves the cursor put
    // on a type mismatch, so the fallback re-reads the same value.
    match reader.read_uint() {
        Ok(u) => {
            let _ = write!(buf, "{u}");
            Ok(())
        }
        Err(_) => {
            let i = reader.read_int()?;
            let _ = write!(buf, "{i}");
            Ok(())
        }
    }
}

/// If `key` names a SID or access field, consume the value at `reader` with the
/// key-derived strategy and return its rendered form. Returns `Ok(None)`
/// **without consuming the value** when the key has no special meaning or the
/// value isn't the shape the strategy needs, so the caller renders it normally.
fn render_special_value(reader: &mut Reader, key: &str) -> peios::Result<Option<String>> {
    if let Some(rendered) = render_sid_field(reader, key)? {
        return Ok(Some(rendered));
    }
    render_access_field(reader, key)
}

/// If `key` names a SID field — suffix `sid` (one SID) or `sids` (an array of
/// SIDs) — render the value at `reader` as canonical string SID(s) like
/// `S-1-5-18`, consuming it. Returns `Ok(None)` without consuming when it isn't
/// a SID field, or the value isn't the shape a SID field should hold, so the
/// caller renders it normally. A `bin` that doesn't parse as a SID falls back
/// to a hex preview rather than hiding.
fn render_sid_field(reader: &mut Reader, key: &str) -> peios::Result<Option<String>> {
    let key = key.to_ascii_lowercase();
    if key.ends_with("sids") {
        if reader.peek() != Some(Type::Array) {
            return Ok(None);
        }
        let n = reader.read_array()?;
        let mut parts = Vec::with_capacity(n);
        for _ in 0..n {
            if reader.peek() == Some(Type::Bin) {
                let b = reader.read_bin()?;
                parts.push(sid_to_string(b).unwrap_or_else(|| hex_preview(b)));
            } else {
                let mut s = String::new();
                compact(reader, &mut s)?;
                parts.push(s);
            }
        }
        Ok(Some(format!("[{}]", parts.join(", "))))
    } else if key.ends_with("sid") {
        if reader.peek() != Some(Type::Bin) {
            return Ok(None);
        }
        let b = reader.read_bin()?;
        Ok(Some(sid_to_string(b).unwrap_or_else(|| hex_preview(b))))
    } else {
        Ok(None)
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
/// at `reader` into `|`-joined right names, consuming it. Returns `Ok(None)`
/// without consuming when it isn't an access field or the value isn't an
/// integer, so the caller renders it normally. An integer too large for a `u32`
/// is rendered as its plain decimal value rather than decoded.
fn render_access_field(reader: &mut Reader, key: &str) -> peios::Result<Option<String>> {
    if !key.to_ascii_lowercase().ends_with("access") || reader.peek() != Some(Type::Int) {
        return Ok(None);
    }
    // Commit to reading: render via mask decode if it fits a u32, else as the
    // plain integer (matching the old fall-through render of an oversized int).
    let mut s = String::new();
    compact_int_into(reader, &mut s, |buf, u| match u32::try_from(u) {
        Ok(m) => buf.push_str(&decode_access_mask(m)),
        Err(_) => {
            let _ = write!(buf, "{u}");
        }
    })?;
    Ok(Some(s))
}

/// Read an integer node and hand its unsigned value to `on_uint`; a negative
/// integer (which cannot be an access mask) is rendered as its plain decimal.
fn compact_int_into(
    reader: &mut Reader,
    buf: &mut String,
    on_uint: impl FnOnce(&mut String, u64),
) -> peios::Result<()> {
    match reader.read_uint() {
        Ok(u) => on_uint(buf, u),
        Err(_) => {
            let i = reader.read_int()?;
            let _ = write!(buf, "{i}");
        }
    }
    Ok(())
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

/// The pretty form of one value: either an inline fragment (a scalar, an empty
/// map, or a scalar-only array — printed on the key's line) or a multi-line
/// block already formatted at its own indent (a non-empty map, or an array that
/// holds a container — printed beneath a `key:` header line).
///
/// Streaming forces this: the renderer can't peek inside a value to ask whether
/// it "expands" without consuming it, so each value is rendered to a buffer in
/// one pass and reports back which shape it took. Buffering one level at a time
/// also supplies the key-column width that the aligned layout needs up front.
enum Pretty {
    /// A one-line fragment to print on the key/index line (no trailing newline).
    Inline(String),
    /// A finished multi-line block (each line indented, trailing newline) to
    /// print under a `key:` / `[i]:` header.
    Block(String),
}

/// Multi-line pretty rendering of a msgpack payload, written to `out` at the
/// given indent. The old `msgpack::Value::render` used `println!`, which would
/// deadlock against the stdout lock the caller already holds; this targets our
/// writer instead.
fn render_payload_pretty(out: &mut impl Write, payload: &[u8], indent: usize) -> io::Result<()> {
    let pad = " ".repeat(indent);
    if payload.is_empty() {
        return writeln!(out, "{pad}(empty)");
    }
    let mut reader = Reader::new(payload);
    match render_value(&mut reader, indent) {
        Ok(rendered) => {
            match rendered {
                Pretty::Inline(s) => writeln!(out, "{pad}{s}")?,
                Pretty::Block(b) => out.write_all(b.as_bytes())?,
            }
            let rest = reader.remaining();
            if rest != 0 {
                writeln!(out, "{pad}(+{rest} trailing byte(s))")?;
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

/// Render the value at `reader`'s cursor in pretty form at `indent`, consuming
/// it. Maps and arrays recurse; scalars render inline via [`compact`].
fn render_value(reader: &mut Reader, indent: usize) -> peios::Result<Pretty> {
    match reader.peek() {
        Some(Type::Map) => render_map(reader, indent),
        Some(Type::Array) => render_array(reader, indent),
        _ => {
            let mut s = String::new();
            compact(reader, &mut s)?;
            Ok(Pretty::Inline(s))
        }
    }
}

/// Pretty-render a map: an empty map is inline `{}`; otherwise an aligned
/// `key   value` block with bare (unquoted) keys, scalar values column-aligned,
/// nested maps/arrays expanded beneath their key, and SID-/access-suffixed keys
/// rendered inline. No `map(N)`/type-name noise.
fn render_map(reader: &mut Reader, indent: usize) -> peios::Result<Pretty> {
    let pad = " ".repeat(indent);
    let n = reader.read_map()?;
    if n == 0 {
        return Ok(Pretty::Inline("{}".to_string()));
    }

    // Pass 1: read every pair, rendering each value into its Pretty form. This
    // also yields the key-column width for the aligned scalar rows.
    let mut rows: Vec<(String, Pretty)> = Vec::with_capacity(n);
    let mut width = 0usize;
    for _ in 0..n {
        let key = key_label(reader)?;
        width = width.max(key.len());
        // SID-/access-suffixed string keys render their value inline regardless
        // of its shape; otherwise recurse on the value.
        let value = match render_special_value(reader, &key)? {
            Some(rendered) => Pretty::Inline(rendered),
            None => render_value(reader, indent + 2)?,
        };
        rows.push((key, value));
    }

    // Pass 2: format the aligned block now that the width is known.
    let mut block = String::new();
    for (key, value) in rows {
        match value {
            Pretty::Inline(s) => {
                let _ = writeln!(block, "{pad}{key:<width$}  {s}");
            }
            Pretty::Block(b) => {
                let _ = writeln!(block, "{pad}{key}:");
                block.push_str(&b);
            }
        }
    }
    Ok(Pretty::Block(block))
}

/// Pretty-render an array: an empty array is inline `[]`; a scalar-only array is
/// inline `[e0, e1, …]`; an array holding any container becomes an indexed
/// block, one element per entry.
fn render_array(reader: &mut Reader, indent: usize) -> peios::Result<Pretty> {
    let pad = " ".repeat(indent);
    let n = reader.read_array()?;
    if n == 0 {
        return Ok(Pretty::Inline("[]".to_string()));
    }

    // Render every element first; whether the array expands depends on whether
    // any element did (streaming can't peek inside without consuming).
    let mut elems: Vec<Pretty> = Vec::with_capacity(n);
    let mut any_block = false;
    for _ in 0..n {
        let p = render_value(reader, indent + 2)?;
        any_block |= matches!(p, Pretty::Block(_));
        elems.push(p);
    }

    if !any_block {
        // Scalar-only array: inline like `compact` would render it.
        let mut s = String::from("[");
        for (i, p) in elems.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            if let Pretty::Inline(frag) = p {
                s.push_str(frag);
            }
        }
        s.push(']');
        return Ok(Pretty::Inline(s));
    }

    // Mixed/container array: an indexed block.
    let mut block = String::new();
    for (i, p) in elems.into_iter().enumerate() {
        match p {
            Pretty::Inline(s) => {
                let _ = writeln!(block, "{pad}[{i}]  {s}");
            }
            Pretty::Block(b) => {
                let _ = writeln!(block, "{pad}[{i}]:");
                block.push_str(&b);
            }
        }
    }
    Ok(Pretty::Block(block))
}

/// A map key as bare text: a string key without quotes; anything else via
/// [`compact`]. Consumes the key value from `reader`.
fn key_label(reader: &mut Reader) -> peios::Result<String> {
    if reader.peek() == Some(Type::Str) {
        Ok(reader.read_str()?.to_string())
    } else {
        let mut s = String::new();
        compact(reader, &mut s)?;
        Ok(s)
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
    use super::{compact, render_access_field, render_sid_field, sid_to_string};
    use peios::msgpack::{Reader, Type, Writer};

    fn sid(authority: u64, subs: &[u32]) -> Vec<u8> {
        let mut b = vec![1u8, subs.len() as u8];
        b.extend_from_slice(&authority.to_be_bytes()[2..]); // 6-byte big-endian authority
        for s in subs {
            b.extend_from_slice(&s.to_le_bytes());
        }
        b
    }

    /// Run the compact renderer over a finished msgpack byte buffer.
    fn compact_bytes(buf: &[u8]) -> String {
        let mut reader = Reader::new(buf);
        let mut out = String::new();
        compact(&mut reader, &mut out).expect("compact render");
        out
    }

    /// Build a one-value payload from a `Writer` closure.
    fn encode(build: impl FnOnce(&mut Writer)) -> Vec<u8> {
        let mut w = Writer::new();
        build(&mut w);
        w.to_bytes().expect("encode")
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
        let buf = encode(|w| {
            w.write_map(4)
                .write_str("user_sid")
                .write_bin(&sid(5, &[18]))
                .write_str("group_sids")
                .write_array(2)
                .write_bin(&sid(1, &[0]))
                .write_bin(&sid(5, &[32, 544]))
                .write_str("policy_sid")
                .write_bin(&sid(5, &[11]))
                .write_str("count")
                .write_uint(3);
        });
        let out = compact_bytes(&buf);
        assert!(out.contains("\"user_sid\": S-1-5-18"), "{out}");
        assert!(
            out.contains("\"group_sids\": [S-1-1-0, S-1-5-32-544]"),
            "{out}"
        );
        assert!(out.contains("\"policy_sid\": S-1-5-11"), "{out}");
        assert!(out.contains("\"count\": 3"), "{out}");
    }

    /// Run `render_sid_field` over a single encoded value: returns the rendered
    /// SID string, or `None` (value left unconsumed, so `remaining` is intact).
    fn sid_field(key: &str, buf: &[u8]) -> Option<String> {
        let mut reader = Reader::new(buf);
        let before = reader.remaining();
        let r = render_sid_field(&mut reader, key).expect("render_sid_field");
        if r.is_none() {
            // A fall-through must not consume the value.
            assert_eq!(reader.remaining(), before, "fall-through consumed value");
        }
        r
    }

    #[test]
    fn non_sid_keys_and_bad_shapes_fall_through() {
        // A non-SID key is not rewritten.
        assert_eq!(sid_field("count", &encode(|w| {
            w.write_uint(1);
        })), None);
        // A `sid` key whose value is not binary falls through to normal render.
        assert_eq!(sid_field("user_sid", &encode(|w| {
            w.write_uint(1);
        })), None);
        // A `sid`-suffixed bin that isn't a valid SID falls back to hex.
        let r = sid_field("weird_sid", &encode(|w| {
            w.write_bin(&[0xde, 0xad]);
        }))
        .unwrap();
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

    /// Run `render_access_field` over a single encoded value; asserts a
    /// fall-through (`None`) leaves the value unconsumed.
    fn access_field(key: &str, buf: &[u8]) -> Option<String> {
        let mut reader = Reader::new(buf);
        let before = reader.remaining();
        let r = render_access_field(&mut reader, key).expect("render_access_field");
        if r.is_none() {
            assert_eq!(reader.remaining(), before, "fall-through consumed value");
        }
        r
    }

    #[test]
    fn access_suffix_fields_decode() {
        assert_eq!(
            access_field("granted_access", &encode(|w| {
                w.write_uint(0x0106_0080);
            }))
            .as_deref(),
            Some("FILE_READ_ATTRIBUTES|READ_CONTROL|WRITE_DAC|ACCESS_SYSTEM_SECURITY")
        );
        // Non-access key, and a wrong-typed value, both fall through.
        assert_eq!(access_field("count", &encode(|w| {
            w.write_uint(5);
        })), None);
        assert_eq!(
            access_field("granted_access", &encode(|w| {
                w.write_str("x");
            })),
            None
        );
    }

    #[test]
    fn pretty_map_aligns_and_expands() {
        use super::render_payload_pretty;
        let buf = encode(|w| {
            w.write_map(3)
                .write_str("a")
                .write_uint(1)
                .write_str("longer_key")
                .write_str("v")
                .write_str("nested")
                .write_map(1)
                .write_str("k")
                .write_uint(2);
        });
        let mut out: Vec<u8> = Vec::new();
        render_payload_pretty(&mut out, &buf, 0).unwrap();
        let s = String::from_utf8(out).unwrap();
        // Scalar rows are column-aligned to the widest key; the nested map is
        // expanded beneath a `nested:` header.
        assert!(s.contains("a           1"), "{s}");
        assert!(s.contains("longer_key  \"v\""), "{s}");
        assert!(s.contains("nested:"), "{s}");
        assert!(s.contains("k  2"), "{s}");
    }

    #[test]
    fn inline_scalar_array_stays_inline() {
        let buf = encode(|w| {
            w.write_array(3).write_uint(1).write_uint(2).write_uint(3);
        });
        assert_eq!(compact_bytes(&buf), "[1, 2, 3]");
    }

    /// Sanity-check the peek/read contract the renderer relies on.
    #[test]
    fn reader_peek_matches_read() {
        let buf = encode(|w| {
            w.write_str("hello");
        });
        let reader = Reader::new(&buf);
        assert_eq!(reader.peek(), Some(Type::Str));
    }
}
