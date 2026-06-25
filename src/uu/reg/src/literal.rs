// Value literals: parse a CLI data token into (type, bytes), and format stored
// bytes back for display (docs/reg-spec.md §3, §5).
//
// Encoding (Peios is UTF-8 throughout, so strings are UTF-8, not UTF-16):
//   SZ / EXPAND_SZ / LINK : UTF-8 bytes + one trailing NUL
//   MULTI_SZ              : each element UTF-8 + NUL, then a final NUL
//   DWORD                 : u32 little-endian (4 bytes)
//   DWORD_BIG_ENDIAN      : u32 big-endian (4 bytes)
//   QWORD                 : u64 little-endian (8 bytes)
//   BINARY                : raw bytes
//   NONE                  : empty
//
// parse() and format() are inverses for these types, so a value round-trips
// through `reg get`/`reg set` without changing type or content.

use crate::error::{Error, Result};
use peios::registry::ValueType;
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
        ValueType::SZ | ValueType::EXPAND_SZ | ValueType::LINK => sz_bytes(rest),
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
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    b
}

fn multi_bytes(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    if !s.is_empty() {
        for elem in split_escaped_commas(s) {
            out.extend_from_slice(elem.as_bytes());
            out.push(0);
        }
    }
    out.push(0); // final terminator
    out
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
    match ty {
        ValueType::NONE => "REG_NONE".into(),
        ValueType::SZ => "REG_SZ".into(),
        ValueType::EXPAND_SZ => "REG_EXPAND_SZ".into(),
        ValueType::BINARY => "REG_BINARY".into(),
        ValueType::DWORD => "REG_DWORD".into(),
        ValueType::DWORD_BIG_ENDIAN => "REG_DWORD_BIG_ENDIAN".into(),
        ValueType::LINK => "REG_LINK".into(),
        ValueType::MULTI_SZ => "REG_MULTI_SZ".into(),
        ValueType::QWORD => "REG_QWORD".into(),
        ValueType::TOMBSTONE => "REG_TOMBSTONE".into(),
        other => format!("REG(0x{:x})", other.0),
    }
}

/// A short keyword name (lowercase, for compact listings / JSON `type` field).
pub fn type_keyword(ty: ValueType) -> String {
    match ty {
        ValueType::NONE => "none".into(),
        ValueType::SZ => "sz".into(),
        ValueType::EXPAND_SZ => "expand".into(),
        ValueType::BINARY => "binary".into(),
        ValueType::DWORD => "dword".into(),
        ValueType::DWORD_BIG_ENDIAN => "dword-be".into(),
        ValueType::LINK => "link".into(),
        ValueType::MULTI_SZ => "multi".into(),
        ValueType::QWORD => "qword".into(),
        ValueType::TOMBSTONE => "tombstone".into(),
        other => format!("0x{:x}", other.0),
    }
}

/// Render value data for the human view (a single concise line where possible).
pub fn format_human(ty: ValueType, data: &[u8]) -> String {
    match ty {
        ValueType::SZ | ValueType::EXPAND_SZ | ValueType::LINK => {
            format!("{:?}", decode_sz(data))
        }
        ValueType::DWORD => match decode_u32_le(data) {
            Some(v) => v.to_string(),
            None => hex(data),
        },
        ValueType::DWORD_BIG_ENDIAN => match decode_u32_be(data) {
            Some(v) => v.to_string(),
            None => hex(data),
        },
        ValueType::QWORD => match decode_u64_le(data) {
            Some(v) => v.to_string(),
            None => hex(data),
        },
        ValueType::MULTI_SZ => format!("[{}]", decode_multi(data).join(", ")),
        ValueType::NONE if data.is_empty() => "(none)".into(),
        _ => hex(data),
    }
}

/// Render value data "bare" for a single `get` — no surrounding quotes, one
/// element per line for MULTI_SZ — so output pipes cleanly.
pub fn format_bare(ty: ValueType, data: &[u8]) -> String {
    match ty {
        ValueType::SZ | ValueType::EXPAND_SZ | ValueType::LINK => decode_sz(data),
        ValueType::DWORD => decode_u32_le(data).map_or_else(|| hex(data), |v| v.to_string()),
        ValueType::DWORD_BIG_ENDIAN => decode_u32_be(data).map_or_else(|| hex(data), |v| v.to_string()),
        ValueType::QWORD => decode_u64_le(data).map_or_else(|| hex(data), |v| v.to_string()),
        ValueType::MULTI_SZ => decode_multi(data).join("\n"),
        ValueType::NONE => String::new(),
        _ => hex(data),
    }
}

/// Render value data for the JSON view (typed where we can decode it).
pub fn format_json(ty: ValueType, data: &[u8]) -> Json {
    let value = match ty {
        ValueType::SZ | ValueType::EXPAND_SZ | ValueType::LINK => json!(decode_sz(data)),
        ValueType::DWORD => decode_u32_le(data).map_or_else(|| json!(hex(data)), |v| json!(v)),
        ValueType::DWORD_BIG_ENDIAN => {
            decode_u32_be(data).map_or_else(|| json!(hex(data)), |v| json!(v))
        }
        ValueType::QWORD => decode_u64_le(data).map_or_else(|| json!(hex(data)), |v| json!(v)),
        ValueType::MULTI_SZ => json!(decode_multi(data)),
        ValueType::NONE => Json::Null,
        _ => json!(hex(data)),
    };
    json!({ "type": type_keyword(ty), "data": value })
}

fn decode_sz(data: &[u8]) -> String {
    let end = data.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
    String::from_utf8_lossy(&data[..end]).into_owned()
}

fn decode_multi(data: &[u8]) -> Vec<String> {
    data.split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

fn decode_u32_le(d: &[u8]) -> Option<u32> {
    (d.len() == 4).then(|| u32::from_le_bytes(d.try_into().unwrap()))
}
fn decode_u32_be(d: &[u8]) -> Option<u32> {
    (d.len() == 4).then(|| u32::from_be_bytes(d.try_into().unwrap()))
}
fn decode_u64_le(d: &[u8]) -> Option<u64> {
    (d.len() == 8).then(|| u64::from_le_bytes(d.try_into().unwrap()))
}

/// Encode stored `(ty, data)` back into a `type:`-prefixed literal token — the
/// inverse of [`parse`] for the text batch format. Always explicit (never
/// relies on inference) so re-`apply` is exact.
pub fn to_token(ty: ValueType, data: &[u8]) -> String {
    match ty {
        ValueType::SZ => format!("sz:{}", decode_sz(data)),
        ValueType::EXPAND_SZ => format!("expand:{}", decode_sz(data)),
        ValueType::LINK => format!("link:{}", decode_sz(data)),
        ValueType::DWORD => format!("dword:{}", decode_u32_le(data).unwrap_or(0)),
        ValueType::DWORD_BIG_ENDIAN => format!("dword-be:{}", decode_u32_be(data).unwrap_or(0)),
        ValueType::QWORD => format!("qword:{}", decode_u64_le(data).unwrap_or(0)),
        ValueType::MULTI_SZ => {
            let parts: Vec<String> = decode_multi(data)
                .into_iter()
                .map(|e| e.replace('\\', r"\\").replace(',', r"\,"))
                .collect();
            format!("multi:{}", parts.join(","))
        }
        ValueType::BINARY => format!("hex:{}", hex(data)),
        ValueType::NONE => "none:".to_string(),
        ValueType::TOMBSTONE => "<tombstone>".to_string(),
        other => format!("0x{:x}:{}", other.0, hex(data)),
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
        assert_eq!(decode_multi(&bytes), vec!["alpha", "beta"]);
        // escaped comma stays in one element
        let (_, b2) = parse(r"multi:a\,b,c").unwrap();
        assert_eq!(decode_multi(&b2), vec!["a,b", "c"]);
    }

    #[test]
    fn hex_parses() {
        let (ty, bytes) = parse("hex:de:ad-be ef").unwrap();
        assert_eq!(ty, ValueType::BINARY);
        assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn sz_has_nul_terminator() {
        let (_, bytes) = parse("sz:hi").unwrap();
        assert_eq!(bytes, b"hi\0");
        assert_eq!(decode_sz(&bytes), "hi");
    }
}
