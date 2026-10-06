// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (libs) libevman revstrm (vars) luid ntfe

//! The event catalogue, as `revstrm` uses it: a path in, the declared type out.
//!
//! The catalogue (PGSS §6.10) declares each field's wire type -- `bin.sid`,
//! `uint.mask`, `uint.time` and so on. `revstrm` formats a payload value by the
//! type its flattened dotted path is declared with, and only falls back to the
//! old key-suffix guesses when there is no catalogue or the path is not in it.
//!
//! The dependency runs one way: `revstrm` reads the catalogue through
//! `libevman`, which knows nothing of the event stream. The pure value
//! formatters live here too, so they can be tested without a msgpack buffer.

use std::fmt::Write as _;
use std::path::Path;

use libevman::catalogue::{Catalogue, Resolution};
use libevman::corpus;

/// The catalogue as loaded for one run. Empty when none is installed.
#[derive(Default)]
pub struct Schema {
    cat: Option<Catalogue>,
}

/// What the catalogue declares for one path.
#[derive(Debug, Clone, Copy)]
pub struct Spec<'a> {
    /// The declared type, array suffix included: `bin.sid[]`.
    pub ty: &'a str,
    /// The field's `values:` line, if it has one.
    pub values: Option<&'a str>,
}

impl Schema {
    /// No catalogue: every path is unknown, so the suffix heuristics apply.
    pub fn none() -> Self {
        Self::default()
    }

    /// Load the catalogue from `/usr/share/evman`, or `EVMAN_DIR` if set. A
    /// missing or unreadable catalogue is not an error: `revstrm` is a debugging
    /// probe and must still show events on a machine without one.
    pub fn load() -> Self {
        Self::load_dir(&corpus::dir())
    }

    /// Load the catalogue from `dir`.
    pub fn load_dir(dir: &Path) -> Self {
        match corpus::read(dir) {
            Ok(sources) if !sources.is_empty() => Self {
                cat: Some(Catalogue::from_sources(&sources)),
            },
            _ => Self::none(),
        }
    }

    /// The declared type of the flattened dotted `path`, if the catalogue
    /// defines it: a field, a variant of one, or a standard attribute whose
    /// type is fixed by its name (`.sid`, `.guid`, `.path`).
    pub fn spec(&self, path: &str) -> Option<Spec<'_>> {
        let cat = self.cat.as_ref()?;
        match cat.resolve(path)? {
            Resolution::Field(e) => spec_of(e),
            // A `-count` of a field is a number of them, not one of them.
            Resolution::Variant { qualifier: "-count", .. } => None,
            Resolution::Variant { field, .. } => spec_of(field),
            Resolution::Attribute { attribute, .. } => {
                let ty = match attribute.as_str() {
                    "sid" => "bin.sid",
                    "guid" => "bin.guid",
                    "path" => "str.path",
                    _ => return None,
                };
                Some(Spec { ty, values: None })
            }
        }
    }

    /// The name the catalogue gives an `emitter.class` value, for the header.
    pub fn emitter_class_name(&self, class: u8) -> Option<String> {
        let spec = self.spec("emitter.class")?;
        enum_name(spec.values?, u64::from(class)).map(str::to_string)
    }
}

fn spec_of(e: &libevman::catalogue::Entry) -> Option<Spec<'_>> {
    Some(Spec {
        ty: e.record.value("type")?,
        values: e.record.value("values"),
    })
}

/// Parse a `values:` line (`0 userspace | 1 kmes`, `0x1 PROT_READ | ...`) into
/// its numbered entries. Entries with no number (a positional list of names)
/// are skipped: they cannot be decoded.
fn numbered_values(values: &str) -> Vec<(u64, &str)> {
    values
        .split('|')
        .filter_map(|item| {
            let (num, name) = item.trim().split_once(char::is_whitespace)?;
            let n = match num.strip_prefix("0x") {
                Some(h) => u64::from_str_radix(h, 16).ok()?,
                None => num.parse().ok()?,
            };
            Some((n, name.trim()))
        })
        .collect()
}

/// The name a numeric enumeration's `values:` line gives `v`.
pub fn enum_name(values: &str, v: u64) -> Option<&str> {
    numbered_values(values)
        .into_iter()
        .find(|&(n, _)| n == v)
        .map(|(_, name)| name)
}

/// A numeric enumeration: `name (v)`, or the plain number when it is not listed.
pub fn format_enum(values: Option<&str>, v: u64) -> String {
    match values.and_then(|vals| enum_name(vals, v)) {
        Some(name) => format!("{name} ({v})"),
        None => v.to_string(),
    }
}

/// A flag set, as `A|B|0x..` using the numbered `values:`. A set the catalogue
/// gives no numbers for cannot be decoded and is shown as hexadecimal.
pub fn format_flags(values: Option<&str>, v: u64) -> String {
    let table = values.map(numbered_values).unwrap_or_default();
    if table.is_empty() {
        return format!("0x{v:x}");
    }
    if v == 0 {
        return "0".to_string();
    }
    let mut rest = v;
    let mut parts: Vec<String> = Vec::new();
    for (bit, name) in table {
        if bit != 0 && rest & bit == bit {
            parts.push(name.to_string());
            rest &= !bit;
        }
    }
    if rest != 0 {
        parts.push(format!("0x{rest:x}"));
    }
    parts.join("|")
}

/// A 16-byte binary GUID (PCDS: Data1..Data3 little-endian) as its canonical
/// string, or `None` if it is not 16 bytes.
pub fn format_guid(b: &[u8]) -> Option<String> {
    let b: &[u8; 16] = b.try_into().ok()?;
    let d1 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let d2 = u16::from_le_bytes([b[4], b[5]]);
    let d3 = u16::from_le_bytes([b[6], b[7]]);
    Some(format!(
        "{d1:08x}-{d2:04x}-{d3:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    ))
}

/// Nanoseconds since the Unix epoch as an RFC 3339 UTC time with nanosecond
/// precision: `2026-10-06T12:34:56.123456789Z`.
pub fn format_time(ns: u64) -> String {
    let secs = ns / 1_000_000_000;
    let nanos = ns % 1_000_000_000;
    let (days, tod) = (secs / 86_400, secs % 86_400);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + u64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{nanos:09}Z",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

/// `n / div` to at most three decimals, trailing zeros trimmed.
fn scaled(n: u64, div: u64) -> String {
    let whole = n / div;
    let frac = u128::from(n % div) * 1000 / u128::from(div);
    let mut s = whole.to_string();
    if frac != 0 {
        let _ = write!(s, ".{frac:03}");
        while s.ends_with('0') {
            s.pop();
        }
    }
    s
}

/// A duration in nanoseconds: `1500000 (1.5ms)`. Under a microsecond, just `Nns`.
pub fn format_duration(ns: u64) -> String {
    const UNITS: [(u64, &str); 4] = [
        (1_000_000_000, "s"),
        (1_000_000, "ms"),
        (1_000, "µs"),
        (1, "ns"),
    ];
    if ns < 1_000 {
        return format!("{ns}ns");
    }
    let (div, unit) = UNITS.iter().find(|(d, _)| ns >= *d).copied().unwrap_or((1, "ns"));
    format!("{ns}ns ({}{unit})", scaled(ns, div))
}

/// A size in bytes: `4096 (4KiB)`. Under a KiB, just the number.
pub fn format_bytes(n: u64) -> String {
    const UNITS: [(u64, &str); 5] = [
        (1 << 40, "TiB"),
        (1 << 30, "GiB"),
        (1 << 20, "MiB"),
        (1 << 10, "KiB"),
        (1, "B"),
    ];
    if n < 1024 {
        return n.to_string();
    }
    let (div, unit) = UNITS.iter().find(|(d, _)| n >= *d).copied().unwrap_or((1, "B"));
    format!("{n} ({}{unit})", scaled(n, div))
}

/// An integrity RID by level name, with the RID.
pub fn format_integrity(rid: u64) -> String {
    let name = match rid {
        0x0000 => "untrusted",
        0x1000 => "low",
        0x2000 => "medium",
        0x2100 => "medium-plus",
        0x3000 => "high",
        0x4000 => "system",
        0x5000 => "protected",
        _ => return format!("0x{rid:x}"),
    };
    format!("{name} (0x{rid:x})")
}

/// A negative errno as `-13 (Permission denied)`.
pub fn format_errno(e: i64) -> String {
    let Some(code) = e.checked_neg().and_then(|n| i32::try_from(n).ok()) else {
        return e.to_string();
    };
    let text = std::io::Error::from_raw_os_error(code).to_string();
    // `Permission denied (os error 13)` -> `Permission denied`.
    let text = text.split(" (os error").next().unwrap_or(&text);
    format!("{e} ({text})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_is_rfc3339_with_nanoseconds() {
        assert_eq!(format_time(0), "1970-01-01T00:00:00.000000000Z");
        // 2024-02-29T23:59:58.5Z, a leap day.
        assert_eq!(
            format_time(1_709_251_198_500_000_000),
            "2024-02-29T23:59:58.500000000Z"
        );
        assert_eq!(
            format_time(1_790_000_000_123_456_789),
            "2026-09-21T14:13:20.123456789Z"
        );
    }

    #[test]
    fn guid_is_mixed_endian() {
        let b = [
            0x04, 0x03, 0x02, 0x01, 0x06, 0x05, 0x08, 0x07, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10,
        ];
        assert_eq!(
            format_guid(&b).as_deref(),
            Some("01020304-0506-0708-090a-0b0c0d0e0f10")
        );
        assert_eq!(format_guid(&b[..15]), None);
    }

    #[test]
    fn duration_and_bytes_scale() {
        assert_eq!(format_duration(999), "999ns");
        assert_eq!(format_duration(1_500_000), "1500000ns (1.5ms)");
        assert_eq!(format_duration(2_000_000_000), "2000000000ns (2s)");
        assert_eq!(format_bytes(512), "512");
        assert_eq!(format_bytes(4096), "4096 (4KiB)");
        assert_eq!(format_bytes(3 << 20), "3145728 (3MiB)");
    }

    #[test]
    fn enums_flags_integrity_errno() {
        let e = "0 userspace | 1 kmes | 2 kacs";
        assert_eq!(format_enum(Some(e), 2), "kacs (2)");
        assert_eq!(format_enum(Some(e), 9), "9");
        assert_eq!(format_enum(None, 9), "9");
        let f = "0x1 PROT_READ | 0x2 PROT_WRITE | 0x4 PROT_EXEC";
        assert_eq!(format_flags(Some(f), 0x3), "PROT_READ|PROT_WRITE");
        assert_eq!(format_flags(Some(f), 0x9), "PROT_READ|0x8");
        assert_eq!(format_flags(Some(f), 0), "0");
        // Names with no numbers cannot be decoded.
        assert_eq!(format_flags(Some("SE_A | SE_B"), 0x6), "0x6");
        assert_eq!(format_integrity(0x2000), "medium (0x2000)");
        assert_eq!(format_integrity(0x1234), "0x1234");
        assert_eq!(format_errno(-13), "-13 (Permission denied)");
    }
}
