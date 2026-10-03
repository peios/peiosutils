// Value literals: parse a CLI data token into (type, bytes), and format stored
// bytes back for display (docs/reg-spec.md §3, §5).
//
// The bytes of each type are `peios::registry::Data`'s: UTF-8 strings with one
// NUL, little-endian numbers. parse() and format() are inverses for these
// types, so a value round-trips through `reg get`/`reg set` without changing
// type or content. Bytes that don't decode as their type (a string that is
// not UTF-8, a number of the wrong length) are shown as hex.

use crate::error::{Error, Result};
use peios::registry::{Data, ValueType};
use serde_json::{json, Value as Json};

/// Parse a CLI data token into a registry `(type, bytes)` pair.
///
/// A `type:` prefix forces the type when the substring before the first `:` is
/// a recognised keyword; otherwise the type is inferred (broad rule, §3.2).
pub fn parse(token: &str) -> Result<(ValueType, Vec<u8>)> {
    if let Some((prefix, rest)) = token.split_once(':') {
        if let Some(ty) = keyword(prefix) {
            return parse_typed(ty, rest);
        }
    }
    Ok(infer(token))
}

/// Map a `type:` keyword to its `ValueType`, or `None` if unrecognised.
fn keyword(s: &str) -> Option<ValueType> {
    match s {
        "sz" => Some(ValueType::SZ),
        "expand" => Some(ValueType::EXPAND_SZ),
        "dword" => Some(ValueType::DWORD),
        "dword-be" => Some(ValueType::DWORD_BIG_ENDIAN),
        "qword" => Some(ValueType::QWORD),
        "multi" => Some(ValueType::MULTI_SZ),
        "hex" | "bin" => Some(ValueType::BINARY),
        "link" => Some(ValueType::LINK),
        "none" => Some(ValueType::NONE),
        _ => None,
    }
}

fn parse_typed(ty: ValueType, rest: &str) -> Result<(ValueType, Vec<u8>)> {
    let bytes = match ty {
        ValueType::SZ => sz_bytes(rest),
        ValueType::EXPAND_SZ => Data::ExpandSz(rest.to_owned()).encode(),
        ValueType::LINK => Data::Link(rest.to_owned()).encode(),
        ValueType::DWORD => parse_u32(rest)?.to_le_bytes().to_vec(),
        ValueType::DWORD_BIG_ENDIAN => parse_u32(rest)?.to_be_bytes().to_vec(),
        ValueType::QWORD => parse_u64(rest)?.to_le_bytes().to_vec(),
        ValueType::MULTI_SZ => multi_bytes(rest),
        ValueType::BINARY => parse_hex(rest)?,
        ValueType::NONE => {
            if !rest.is_empty() {
                return Err(Error::InvalidSpec("none: takes no data".into()));
            }
            Vec::new()
        }
        _ => return Err(Error::InvalidSpec("unsupported value type".into())),
    };
    Ok((ty, bytes))
}

/// Broad inference (§3.2): any all-digit token → DWORD/QWORD by magnitude;
/// `0x…` → DWORD/QWORD by width; everything else → SZ. Leading zeros are lost
/// (the accepted footgun).
fn infer(token: &str) -> (ValueType, Vec<u8>) {
    if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
        if !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            if let Ok(v) = u32::from_str_radix(hex, 16) {
                return (ValueType::DWORD, v.to_le_bytes().to_vec());
            }
            if let Ok(v) = u64::from_str_radix(hex, 16) {
                return (ValueType::QWORD, v.to_le_bytes().to_vec());
            }
        }
    } else if !token.is_empty() && token.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(v) = token.parse::<u32>() {
            return (ValueType::DWORD, v.to_le_bytes().to_vec());
        }
        if let Ok(v) = token.parse::<u64>() {
            return (ValueType::QWORD, v.to_le_bytes().to_vec());
        }
    }
    (ValueType::SZ, sz_bytes(token))
}

fn sz_bytes(s: &str) -> Vec<u8> {
    Data::Sz(s.to_owned()).encode()
}

fn multi_bytes(s: &str) -> Vec<u8> {
    let list = if s.is_empty() { Vec::new() } else { split_escaped_commas(s) };
    Data::MultiSz(list).encode()
}

/// Split on commas, honouring `\,` as a literal comma and `\\` as a backslash.
fn split_escaped_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(',') => cur.push(','),
                Some('\\') => cur.push('\\'),
                Some(other) => {
                    cur.push('\\');
                    cur.push(other);
                }
                None => cur.push('\\'),
            },
            ',' => {
                out.push(std::mem::take(&mut cur));
            }
            other => cur.push(other),
        }
    }
    out.push(cur);
    out
}

fn parse_u32(s: &str) -> Result<u32> {
    let v = parse_int(s)?;
    u32::try_from(v).map_err(|_| Error::InvalidSpec(format!("{s}: does not fit in a DWORD (u32)")))
}

fn parse_u64(s: &str) -> Result<u64> {
    parse_int(s)
}

fn parse_int(s: &str) -> Result<u64> {
    let s = s.trim();
    let parsed = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(h, 16)
    } else {
        s.parse::<u64>()
    };
    parsed.map_err(|_| Error::InvalidSpec(format!("{s:?}: not an integer")))
}

fn parse_hex(s: &str) -> Result<Vec<u8>> {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, ':' | '-' | ' ' | '\t' | '\n'))
        .collect();
    if cleaned.len() % 2 != 0 {
        return Err(Error::InvalidSpec(
            "hex: needs an even number of digits".into(),
        ));
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| Error::InvalidSpec(format!("hex: invalid byte {:?}", &cleaned[i..i + 2])))
        })
        .collect()
}

// --------------------------------------------------------------------------
// Formatting (stored bytes → display).
// --------------------------------------------------------------------------

/// The canonical `REG_*` name for a value type.
pub fn type_name(ty: ValueType) -> String {
    ty.name().map_or_else(|| format!("REG(0x{:x})", ty.0), str::to_owned)
}

/// A short keyword name (lowercase, for compact listings / JSON `type` field).
pub fn type_keyword(ty: ValueType) -> String {
    libreg::keyword(ty)
}

/// Render value data for the human view (a single concise line where possible).
pub fn format_human(ty: ValueType, data: &[u8]) -> String {
    match Data::decode(ty, data) {
        Data::Sz(s) | Data::ExpandSz(s) | Data::Link(s) => format!("{s:?}"),
        Data::Dword(v) | Data::DwordBigEndian(v) => v.to_string(),
        Data::Qword(v) => v.to_string(),
        Data::MultiSz(list) => format!("[{}]", list.join(", ")),
        Data::None => "(none)".into(),
        Data::Binary(bytes) | Data::Raw(_, bytes) => hex(&bytes),
    }
}

/// Render value data "bare" for a single `get` — no surrounding quotes, one
/// element per line for MULTI_SZ — so output pipes cleanly.
pub fn format_bare(ty: ValueType, data: &[u8]) -> String {
    match Data::decode(ty, data) {
        Data::Sz(s) | Data::ExpandSz(s) | Data::Link(s) => s,
        Data::Dword(v) | Data::DwordBigEndian(v) => v.to_string(),
        Data::Qword(v) => v.to_string(),
        Data::MultiSz(list) => list.join("\n"),
        Data::None => String::new(),
        Data::Binary(bytes) | Data::Raw(_, bytes) => hex(&bytes),
    }
}

/// Render value data for the JSON view (typed where we can decode it).
pub fn format_json(ty: ValueType, data: &[u8]) -> Json {
    // As the batch document has it: data that doesn't fit its type goes as
    // `hex`, its bytes exactly.
    match libreg::data_json(ty, data) {
        Some(value) => json!({ "type": type_keyword(ty), "data": value }),
        None => json!({ "type": type_keyword(ty), "hex": libreg::hex(data) }),
    }
}

/// Encode stored `(ty, data)` back into a `type:`-prefixed literal token — the
/// inverse of [`parse`] for the text batch format. Always explicit (never
/// relies on inference) so re-`apply` is exact.
pub fn to_token(ty: ValueType, data: &[u8]) -> String {
    match Data::decode(ty, data) {
        Data::Sz(s) => format!("sz:{s}"),
        Data::ExpandSz(s) => format!("expand:{s}"),
        Data::Link(s) => format!("link:{s}"),
        Data::Dword(v) => format!("dword:{v}"),
        Data::DwordBigEndian(v) => format!("dword-be:{v}"),
        Data::Qword(v) => format!("qword:{v}"),
        Data::MultiSz(list) => {
            let parts: Vec<String> = list
                .into_iter()
                .map(|e| e.replace('\\', r"\\").replace(',', r"\,"))
                .collect();
            format!("multi:{}", parts.join(","))
        }
        Data::Binary(bytes) => format!("hex:{}", hex(&bytes)),
        Data::None => "none:".to_string(),
        Data::Raw(ValueType::TOMBSTONE, _) => "<tombstone>".to_string(),
        Data::Raw(other, bytes) => format!("0x{:x}:{}", other.0, hex(&bytes)),
    }
}

/// Whether inferring `ty` from `token` (no explicit `type:` prefix) is a
/// "surprising" coercion worth always echoing, even under `--quiet` (spec O5):
/// a leading-zero or hex token that silently became a number.
pub fn is_surprising_coercion(token: &str, ty: ValueType) -> bool {
    let numeric = matches!(ty, ValueType::DWORD | ValueType::QWORD);
    let explicit = token
        .split_once(':')
        .is_some_and(|(p, _)| keyword(p).is_some());
    let looks_textual = token.starts_with("0x")
        || token.starts_with("0X")
        || (token.len() > 1 && token.starts_with('0'));
    numeric && !explicit && looks_textual
}

/// Lowercase hex with no separators.
pub fn hex(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 2);
    for b in data {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(token: &str) -> (String, String) {
        let (ty, bytes) = parse(token).unwrap();
        (type_keyword(ty), format_human(ty, &bytes))
    }

    #[test]
    fn infer_numbers_broadly() {
        assert_eq!(rt("4096"), ("dword".into(), "4096".into()));
        assert_eq!(rt("007"), ("dword".into(), "7".into())); // zeros lost
        assert_eq!(rt("0x2A"), ("dword".into(), "42".into()));
        assert_eq!(rt("9000000000"), ("qword".into(), "9000000000".into()));
    }

    #[test]
    fn infer_strings() {
        assert_eq!(rt("hello"), ("sz".into(), "\"hello\"".into()));
        assert_eq!(rt("http://x"), ("sz".into(), "\"http://x\"".into()));
    }

    #[test]
    fn explicit_overrides_inference() {
        assert_eq!(rt("sz:4096"), ("sz".into(), "\"4096\"".into()));
        assert_eq!(rt("sz:dword:42"), ("sz".into(), "\"dword:42\"".into()));
        assert_eq!(rt("qword:5"), ("qword".into(), "5".into()));
    }

    #[test]
    fn multi_sz_roundtrip() {
        let (ty, bytes) = parse("multi:alpha,beta").unwrap();
        assert_eq!(ty, ValueType::MULTI_SZ);
        assert_eq!(format_bare(ty, &bytes), "alpha\nbeta");
        // escaped comma stays in one element
        let (_, b2) = parse(r"multi:a\,b,c").unwrap();
        assert_eq!(format_bare(ty, &b2), "a,b\nc");
    }

    #[test]
    fn hex_parses() {
        let (ty, bytes) = parse("hex:de:ad-be ef").unwrap();
        assert_eq!(ty, ValueType::BINARY);
        assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn sz_has_nul_terminator() {
        let (ty, bytes) = parse("sz:hi").unwrap();
        assert_eq!(bytes, b"hi\0");
        assert_eq!(format_bare(ty, &bytes), "hi");
    }

    #[test]
    fn bytes_that_do_not_fit_their_type_show_as_hex() {
        assert_eq!(format_human(ValueType::SZ, b"\xff\0"), "ff00");
        assert_eq!(format_human(ValueType::DWORD, &[1, 2, 3]), "010203");
        assert_eq!(format_json(ValueType::SZ, b"\xff\0")["hex"], "ff00");
        assert_eq!(to_token(ValueType::DWORD, &[1, 2, 3]), "0x4:010203");
    }
}
