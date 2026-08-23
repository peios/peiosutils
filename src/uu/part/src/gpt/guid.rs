// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! GUIDs as GPT stores them.
//!
//! The one thing to know about a GUID on disk is that it is **mixed-endian**.
//! The textual form `01020304-0506-0708-090A-0B0C0D0E0F10` is written as five
//! groups, and GPT stores the first three little-endian and the last two
//! big-endian:
//!
//! ```text
//!   text:  01020304 - 0506 - 0708 - 090A - 0B0C0D0E0F10
//!   bytes: 04030201   0605   0807   090A   0B0C0D0E0F10
//!          \__LE__/   \LE/   \LE/   \_____BE_________/
//! ```
//!
//! Getting this backwards produces a table that looks plausible in a hex dump
//! and is rejected — or worse, silently misread — by every other tool, so it is
//! tested against the type GUIDs whose byte forms are published.

use std::fmt;

use rand::Rng;

/// A 128-bit GUID, held in its **on-disk** byte order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Guid(pub [u8; 16]);

impl Guid {
    /// The all-zero GUID, which GPT uses to mean "this entry is unused".
    pub const ZERO: Guid = Guid([0; 16]);

    /// Parse the canonical textual form, with or without surrounding braces.
    pub fn parse(s: &str) -> Option<Guid> {
        let s = s.trim();
        let s = s.strip_prefix('{').and_then(|r| r.strip_suffix('}')).unwrap_or(s);
        let f: Vec<&str> = s.split('-').collect();
        if f.len() != 5 || f[0].len() != 8 || f[1].len() != 4 || f[2].len() != 4 || f[3].len() != 4 || f[4].len() != 12 {
            return None;
        }
        let d1 = u32::from_str_radix(f[0], 16).ok()?;
        let d2 = u16::from_str_radix(f[1], 16).ok()?;
        let d3 = u16::from_str_radix(f[2], 16).ok()?;

        let mut out = [0u8; 16];
        out[0..4].copy_from_slice(&d1.to_le_bytes());
        out[4..6].copy_from_slice(&d2.to_le_bytes());
        out[6..8].copy_from_slice(&d3.to_le_bytes());
        // Groups 4 and 5 are stored big-endian, i.e. exactly as written.
        hex_into(f[3], &mut out[8..10])?;
        hex_into(f[4], &mut out[10..16])?;
        Some(Guid(out))
    }

    /// A random version-4 GUID.
    ///
    /// GPT wants every disk and every partition uniquely identified, and unlike
    /// a filesystem UUID these are what the boot path resolves `root=` against —
    /// so they come from the OS CSPRNG rather than anything derived from time or
    /// device identity.
    pub fn random() -> Guid {
        let mut b = [0u8; 16];
        rand::rng().fill_bytes(&mut b);
        // Version 4 and RFC 4122 variant, applied to the *textual* fields —
        // which are bytes 7 and 8 on disk because of the mixed endianness.
        b[7] = (b[7] & 0x0f) | 0x40;
        b[8] = (b[8] & 0x3f) | 0x80;
        Guid(b)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn from_bytes(b: &[u8]) -> Option<Guid> {
        let mut out = [0u8; 16];
        out.copy_from_slice(b.get(..16)?);
        Some(Guid(out))
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = &self.0;
        let d1 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let d2 = u16::from_le_bytes([b[4], b[5]]);
        let d3 = u16::from_le_bytes([b[6], b[7]]);
        write!(f, "{d1:08X}-{d2:04X}-{d3:04X}-")?;
        for x in &b[8..10] {
            write!(f, "{x:02X}")?;
        }
        write!(f, "-")?;
        for x in &b[10..16] {
            write!(f, "{x:02X}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guid({self})")
    }
}

fn hex_into(s: &str, out: &mut [u8]) -> Option<()> {
    if s.len() != out.len() * 2 {
        return None;
    }
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ESP type GUID is the best possible fixture: its byte form appears in
    /// the UEFI spec and in every partitioning tool's source, so a mixed-endian
    /// mistake cannot hide behind self-consistency.
    #[test]
    fn esp_type_guid_matches_the_published_byte_form() {
        let g = Guid::parse("C12A7328-F81F-11D2-BA4B-00A0C93EC93B").unwrap();
        assert_eq!(
            g.0,
            [
                0x28, 0x73, 0x2A, 0xC1, // C12A7328 little-endian
                0x1F, 0xF8, // F81F      little-endian
                0xD2, 0x11, // 11D2      little-endian
                0xBA, 0x4B, // BA4B      big-endian (as written)
                0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B, // big-endian
            ]
        );
    }

    #[test]
    fn linux_filesystem_type_guid_matches() {
        let g = Guid::parse("0FC63DAF-8483-4772-8E79-3D69D8477DE4").unwrap();
        assert_eq!(
            g.0,
            [
                0xAF, 0x3D, 0xC6, 0x0F, 0x83, 0x84, 0x72, 0x47, 0x8E, 0x79, 0x3D, 0x69, 0xD8, 0x47,
                0x7D, 0xE4,
            ]
        );
    }

    #[test]
    fn round_trips_through_text() {
        for s in [
            "C12A7328-F81F-11D2-BA4B-00A0C93EC93B",
            "0FC63DAF-8483-4772-8E79-3D69D8477DE4",
            "00000000-0000-0000-0000-000000000000",
            "FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF",
        ] {
            assert_eq!(Guid::parse(s).unwrap().to_string(), s);
        }
    }

    #[test]
    fn accepts_braces_and_lowercase_and_renders_uppercase() {
        let a = Guid::parse("{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}").unwrap();
        let b = Guid::parse("C12A7328-F81F-11D2-BA4B-00A0C93EC93B").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.to_string(), "C12A7328-F81F-11D2-BA4B-00A0C93EC93B");
    }

    #[test]
    fn rejects_malformed() {
        for s in [
            "",
            "not-a-guid",
            "C12A7328-F81F-11D2-BA4B",                       // too few groups
            "C12A7328-F81F-11D2-BA4B-00A0C93EC93B-extra",    // too many
            "C12A732-F81F-11D2-BA4B-00A0C93EC93B",           // short group
            "G12A7328-F81F-11D2-BA4B-00A0C93EC93B",          // non-hex
        ] {
            assert!(Guid::parse(s).is_none(), "should reject {s:?}");
        }
    }

    #[test]
    fn zero_is_recognised() {
        assert!(Guid::ZERO.is_zero());
        assert!(Guid::parse("00000000-0000-0000-0000-000000000000").unwrap().is_zero());
        assert!(!Guid::random().is_zero());
    }

    #[test]
    fn random_guids_are_v4_and_distinct() {
        let a = Guid::random();
        let b = Guid::random();
        assert_ne!(a, b);
        // Version nibble lives in the third text group's high byte, which is
        // on-disk byte 7 thanks to the little-endian storage of that group.
        assert_eq!(a.0[7] & 0xf0, 0x40, "version must be 4: {a}");
        assert_eq!(a.0[8] & 0xc0, 0x80, "variant must be RFC 4122: {a}");
    }
}
