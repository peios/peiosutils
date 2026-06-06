// Permission parsing — file access masks.
//
// Three input forms (any may mix in `--perms`):
//
//   chmod-shaped letters:  `r`, `w`, `x`, `d`, `m`, `f`, `c`, `o`
//   long words:            `read`, `write`, `execute`, `delete`, `modify`,
//                          `full`, `change-perms`, `take-owner`
//   advanced low-16 names: `read-data`, `write-data`, `append`, `read-ea`,
//                          `write-ea`, `traverse`, `delete-child`,
//                          `read-attrs`, `write-attrs`
//   raw hex:               `0x1F01FF`
//
// `rwx` (no separator) parses as a sequence of single letters.
// `r,w,x` and `read,write,execute` parse as a comma-separated list.

use crate::error::{Error, Result};

// File composites from MS-DTYP §2.5.1.1 (SDDL).
const FILE_ALL: u32 = 0x001F_01FF;
const FILE_READ: u32 = 0x0012_0089;
const FILE_WRITE: u32 = 0x0012_0116;
const FILE_EXECUTE: u32 = 0x0012_00A0;

// Standard rights from peios-uapi (re-imported for clarity).
const DELETE: u32 = 0x0001_0000;
const WRITE_DAC: u32 = 0x0004_0000;
const WRITE_OWNER: u32 = 0x0008_0000;
const READ_CONTROL: u32 = 0x0002_0000;
const SYNCHRONIZE: u32 = 0x0010_0000;

// Composite for "modify": read + write + execute + delete.
const MODIFY: u32 = FILE_READ | FILE_WRITE | FILE_EXECUTE | DELETE;

// File-specific low-16 bits (MS-DTYP §2.4.3 ACE_FILE_OBJECT_ACCESS).
const FILE_READ_DATA: u32 = 0x0001;
const FILE_WRITE_DATA: u32 = 0x0002;
const FILE_APPEND_DATA: u32 = 0x0004;
const FILE_READ_EA: u32 = 0x0008;
const FILE_WRITE_EA: u32 = 0x0010;
const FILE_EXECUTE_BIT: u32 = 0x0020;
const FILE_DELETE_CHILD: u32 = 0x0040;
const FILE_READ_ATTRIBUTES: u32 = 0x0080;
const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;

/// One-letter shorthand → mask.
fn letter(c: char) -> Option<u32> {
    Some(match c {
        'r' | 'R' => FILE_READ,
        'w' | 'W' => FILE_WRITE,
        'x' | 'X' => FILE_EXECUTE,
        'd' | 'D' => DELETE,
        'm' | 'M' => MODIFY,
        'f' | 'F' => FILE_ALL,
        'c' | 'C' => WRITE_DAC,
        'o' | 'O' => WRITE_OWNER,
        _ => return None,
    })
}

/// Long word → mask.
fn word(w: &str) -> Option<u32> {
    Some(match w {
        "read" => FILE_READ,
        "write" => FILE_WRITE,
        "execute" => FILE_EXECUTE,
        "delete" => DELETE,
        "modify" => MODIFY,
        "full" | "all" => FILE_ALL,
        "change-perms" | "write-dac" => WRITE_DAC,
        "take-owner" | "write-owner" => WRITE_OWNER,
        "read-control" => READ_CONTROL,
        "synchronize" | "sync" => SYNCHRONIZE,
        "read-data" => FILE_READ_DATA,
        "write-data" => FILE_WRITE_DATA,
        "append" | "append-data" => FILE_APPEND_DATA,
        "read-ea" => FILE_READ_EA,
        "write-ea" => FILE_WRITE_EA,
        "traverse" | "execute-bit" => FILE_EXECUTE_BIT,
        "delete-child" => FILE_DELETE_CHILD,
        "read-attrs" | "read-attributes" => FILE_READ_ATTRIBUTES,
        "write-attrs" | "write-attributes" => FILE_WRITE_ATTRIBUTES,
        _ => return None,
    })
}

/// Parse a perms spec — comma-separated tokens, or a bare letter run, or
/// a `0x...` hex mask. Returns the OR of all matched bits.
pub fn parse(s: &str) -> Result<u32> {
    let t = s.trim();
    if t.is_empty() {
        return Err(Error::Usage("empty perms".into()));
    }
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16)
            .map_err(|_| Error::Usage(format!("bad hex perms `{t}`")));
    }
    if t.contains(',') {
        let mut mask = 0u32;
        for piece in t.split(',') {
            mask |= parse_one(piece.trim())?;
        }
        return Ok(mask);
    }
    // No comma: try as one whole token first (e.g. "read"), then as
    // a letter-run (e.g. "rwx").
    if let Some(m) = word(t) {
        return Ok(m);
    }
    let mut mask = 0u32;
    for c in t.chars() {
        if let Some(m) = letter(c) {
            mask |= m;
        } else {
            return Err(Error::Usage(format!("unknown perm `{c}` in `{t}`")));
        }
    }
    Ok(mask)
}

fn parse_one(tok: &str) -> Result<u32> {
    if let Some(m) = word(tok) {
        return Ok(m);
    }
    if tok.chars().count() == 1 {
        if let Some(m) = letter(tok.chars().next().unwrap()) {
            return Ok(m);
        }
    }
    Err(Error::Usage(format!("unknown perm `{tok}`")))
}

/// Render a mask back to canonical short form (letters preferred,
/// hex-fallback when bits don't fit a named code). Used by `sd show`.
pub fn render(mask: u32) -> String {
    let mut bits = mask;
    let mut out: Vec<&'static str> = Vec::new();
    // Composites first (longest-match wins) — order matters here.
    for &(name, m) in &[
        ("f", FILE_ALL),
        ("m", MODIFY),
        ("r", FILE_READ),
        ("w", FILE_WRITE),
        ("x", FILE_EXECUTE),
    ] {
        if bits & m == m {
            out.push(name);
            bits &= !m;
        }
    }
    for &(name, m) in &[
        ("d", DELETE),
        ("c", WRITE_DAC),
        ("o", WRITE_OWNER),
        ("rc", READ_CONTROL),
        ("sync", SYNCHRONIZE),
    ] {
        if bits & m != 0 {
            out.push(name);
            bits &= !m;
        }
    }
    let mut s = out.join(",");
    if bits != 0 {
        if !s.is_empty() {
            s.push(',');
        }
        s.push_str(&format!("0x{bits:x}"));
    }
    if s.is_empty() {
        "(none)".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_letter_run() {
        assert_eq!(parse("rwx").unwrap(), FILE_READ | FILE_WRITE | FILE_EXECUTE);
    }

    #[test]
    fn parse_comma_words() {
        assert_eq!(
            parse("read,write,execute").unwrap(),
            FILE_READ | FILE_WRITE | FILE_EXECUTE
        );
    }

    #[test]
    fn parse_hex() {
        assert_eq!(parse("0x1F01FF").unwrap(), FILE_ALL);
    }

    #[test]
    fn parse_modify_is_rwx_plus_delete() {
        assert_eq!(parse("m").unwrap(), MODIFY);
        assert_eq!(parse("modify").unwrap(), MODIFY);
    }

    #[test]
    fn parse_advanced_bit_names() {
        assert_eq!(parse("read-data").unwrap(), FILE_READ_DATA);
        assert_eq!(parse("delete-child").unwrap(), FILE_DELETE_CHILD);
    }

    #[test]
    fn render_full_is_f() {
        assert_eq!(render(FILE_ALL), "f");
    }

    #[test]
    fn render_read_is_r() {
        assert_eq!(render(FILE_READ), "r");
    }

    #[test]
    fn render_unknown_bits_fall_back_to_hex() {
        let mask = FILE_READ | 0x0000_0040; // FR + an advanced bit
        let s = render(mask);
        assert!(s.starts_with("r"));
        assert!(s.contains("0x"));
    }

    #[test]
    fn rejects_unknown_letter() {
        assert!(parse("q").is_err());
    }
}
