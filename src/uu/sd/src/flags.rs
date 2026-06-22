// ACE flag parsing (CI/OI/NP/IO/SA/FA).

use crate::error::{Error, Result};

// ACE flag wire bits (u8), from the peios-sys bindgen constants.
const ACE_FLAG_OBJECT_INHERIT: u8 = peios_sys::KACS_ACE_FLAG_OBJECT_INHERIT as u8;
const ACE_FLAG_CONTAINER_INHERIT: u8 = peios_sys::KACS_ACE_FLAG_CONTAINER_INHERIT as u8;
const ACE_FLAG_NO_PROPAGATE_INHERIT: u8 = peios_sys::KACS_ACE_FLAG_NO_PROPAGATE_INHERIT as u8;
const ACE_FLAG_INHERIT_ONLY: u8 = peios_sys::KACS_ACE_FLAG_INHERIT_ONLY as u8;
const ACE_FLAG_INHERITED: u8 = peios_sys::KACS_ACE_FLAG_INHERITED as u8;
const ACE_FLAG_SUCCESSFUL_ACCESS: u8 = peios_sys::KACS_ACE_FLAG_SUCCESSFUL_ACCESS as u8;
const ACE_FLAG_FAILED_ACCESS: u8 = peios_sys::KACS_ACE_FLAG_FAILED_ACCESS as u8;

/// Parse a comma-separated flag list (`CI,OI,NP,IO,SA,FA`). The literal
/// `none` collapses to 0.
pub fn parse(s: &str) -> Result<u8> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        return Ok(0);
    }
    let mut out = 0u8;
    for piece in t.split(',') {
        out |= one(piece.trim())?;
    }
    Ok(out)
}

fn one(code: &str) -> Result<u8> {
    Ok(match code.to_ascii_uppercase().as_str() {
        "CI" => ACE_FLAG_CONTAINER_INHERIT,
        "OI" => ACE_FLAG_OBJECT_INHERIT,
        "NP" => ACE_FLAG_NO_PROPAGATE_INHERIT,
        "IO" => ACE_FLAG_INHERIT_ONLY,
        "ID" => ACE_FLAG_INHERITED, // accepted on input; ignored by writes
        "SA" => ACE_FLAG_SUCCESSFUL_ACCESS,
        "FA" => ACE_FLAG_FAILED_ACCESS,
        other => return Err(Error::Usage(format!("unknown ACE flag `{other}`"))),
    })
}

/// Render a flags byte to comma-separated codes. Empty → "".
pub fn render(f: u8) -> String {
    const NAMES: &[(u8, &str)] = &[
        (ACE_FLAG_CONTAINER_INHERIT, "CI"),
        (ACE_FLAG_OBJECT_INHERIT, "OI"),
        (ACE_FLAG_NO_PROPAGATE_INHERIT, "NP"),
        (ACE_FLAG_INHERIT_ONLY, "IO"),
        (ACE_FLAG_INHERITED, "ID"),
        (ACE_FLAG_SUCCESSFUL_ACCESS, "SA"),
        (ACE_FLAG_FAILED_ACCESS, "FA"),
    ];
    NAMES
        .iter()
        .filter(|(b, _)| f & *b != 0)
        .map(|(_, n)| *n)
        .collect::<Vec<_>>()
        .join(",")
}

/// Default flags for a new ACE on the given object kind. Per design doc:
/// directories default to `CI,OI`; files default to none.
pub fn default_for(kind: TargetKind) -> u8 {
    match kind {
        TargetKind::Directory => ACE_FLAG_CONTAINER_INHERIT | ACE_FLAG_OBJECT_INHERIT,
        TargetKind::File => 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    File,
    Directory,
}

impl TargetKind {
    pub fn from_path(path: &str) -> std::io::Result<Self> {
        let md = std::fs::metadata(path)?;
        Ok(if md.is_dir() {
            TargetKind::Directory
        } else {
            TargetKind::File
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codes() {
        assert_eq!(
            parse("CI,OI").unwrap(),
            ACE_FLAG_CONTAINER_INHERIT | ACE_FLAG_OBJECT_INHERIT
        );
    }

    #[test]
    fn none_is_zero() {
        assert_eq!(parse("none").unwrap(), 0);
        assert_eq!(parse("").unwrap(), 0);
    }

    #[test]
    fn round_trip() {
        let f = ACE_FLAG_CONTAINER_INHERIT | ACE_FLAG_INHERIT_ONLY;
        assert_eq!(parse(&render(f)).unwrap(), f);
    }
}
