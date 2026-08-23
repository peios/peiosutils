// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! A GPT partition entry (UEFI 2.10 §5.3.3).
//!
//! 128 bytes: two GUIDs, an inclusive LBA range, an attribute word, and a name
//! of up to 36 UTF-16LE code units.
//!
//! Two details bite:
//!
//! * **`ending_lba` is inclusive.** A 2048-sector partition starting at 2048
//!   ends at 4095, not 4096. An off-by-one here overlaps the next partition,
//!   which no tool will warn about until something overwrites something else.
//! * **The name is UTF-16LE, fixed at 36 code units.** Characters outside the
//!   BMP cost two units each, so the limit is in code units, not characters.

use super::guid::Guid;
use crate::error::{PartError, Result};

/// Name capacity in UTF-16 code units (72 bytes).
pub const NAME_UNITS: usize = 36;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GptEntry {
    pub type_guid: Guid,
    pub unique_guid: Guid,
    pub starting_lba: u64,
    /// Inclusive — the last sector belonging to this partition.
    pub ending_lba: u64,
    pub attributes: u64,
    pub name: String,
}

impl GptEntry {
    pub fn is_used(&self) -> bool {
        !self.type_guid.is_zero()
    }

    /// Length in sectors. Zero for an unused entry.
    pub fn sectors(&self) -> u64 {
        if self.is_used() && self.ending_lba >= self.starting_lba {
            self.ending_lba - self.starting_lba + 1
        } else {
            0
        }
    }

    /// Does this entry's sector range intersect `[start, end]` (inclusive)?
    pub fn overlaps(&self, start: u64, end: u64) -> bool {
        self.is_used() && self.starting_lba <= end && start <= self.ending_lba
    }

    pub fn to_bytes(&self) -> [u8; 128] {
        let mut b = [0u8; 128];
        b[0..16].copy_from_slice(self.type_guid.as_bytes());
        b[16..32].copy_from_slice(self.unique_guid.as_bytes());
        b[32..40].copy_from_slice(&self.starting_lba.to_le_bytes());
        b[40..48].copy_from_slice(&self.ending_lba.to_le_bytes());
        b[48..56].copy_from_slice(&self.attributes.to_le_bytes());
        // Truncation is at encode time rather than being rejected, because the
        // caller has already been told the limit by `validate_name`; this is the
        // belt to that braces.
        for (i, unit) in self.name.encode_utf16().take(NAME_UNITS).enumerate() {
            let off = 56 + i * 2;
            b[off..off + 2].copy_from_slice(&unit.to_le_bytes());
        }
        b
    }

    pub fn parse(b: &[u8]) -> Option<GptEntry> {
        if b.len() < 128 {
            return None;
        }
        let mut units = Vec::with_capacity(NAME_UNITS);
        for i in 0..NAME_UNITS {
            let off = 56 + i * 2;
            let u = u16::from_le_bytes([b[off], b[off + 1]]);
            // NUL terminates; the spec pads with zeroes.
            if u == 0 {
                break;
            }
            units.push(u);
        }
        Some(GptEntry {
            type_guid: Guid::from_bytes(&b[0..16])?,
            unique_guid: Guid::from_bytes(&b[16..32])?,
            starting_lba: u64::from_le_bytes(b[32..40].try_into().ok()?),
            ending_lba: u64::from_le_bytes(b[40..48].try_into().ok()?),
            attributes: u64::from_le_bytes(b[48..56].try_into().ok()?),
            name: String::from_utf16_lossy(&units),
        })
    }
}

/// Reject a name that cannot be stored, rather than silently truncating it.
///
/// Silent truncation is the wrong default here: a partition name is how a human
/// identifies the thing they are about to format, and quietly shortening it
/// makes the label on screen disagree with the label on disk.
pub fn validate_name(name: &str) -> Result<()> {
    let units = name.encode_utf16().count();
    if units > NAME_UNITS {
        return Err(PartError::Usage(format!(
            "partition name is {units} UTF-16 code units; the limit is {NAME_UNITS}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn esp() -> Guid {
        Guid::parse("C12A7328-F81F-11D2-BA4B-00A0C93EC93B").unwrap()
    }

    fn sample() -> GptEntry {
        GptEntry {
            type_guid: esp(),
            unique_guid: Guid::parse("01020304-0506-0708-090A-0B0C0D0E0F10").unwrap(),
            starting_lba: 2048,
            ending_lba: 1_050_623,
            attributes: 0,
            name: "EFI system partition".into(),
        }
    }

    #[test]
    fn round_trips() {
        let e = sample();
        assert_eq!(GptEntry::parse(&e.to_bytes()).unwrap(), e);
    }

    #[test]
    fn fields_land_at_their_spec_offsets() {
        let b = sample().to_bytes();
        assert_eq!(&b[0..16], esp().as_bytes());
        assert_eq!(u64::from_le_bytes(b[32..40].try_into().unwrap()), 2048);
        assert_eq!(u64::from_le_bytes(b[40..48].try_into().unwrap()), 1_050_623);
        // "E" as UTF-16LE.
        assert_eq!(&b[56..58], &[0x45, 0x00]);
        assert_eq!(b.len(), 128);
    }

    /// The inclusive-end convention, stated as a test so it cannot drift: a
    /// 512 MiB ESP at 512-byte sectors is 1048576 sectors, 2048..=1050623.
    #[test]
    fn ending_lba_is_inclusive() {
        let e = sample();
        assert_eq!(e.sectors(), 1_048_576);
        assert_eq!(e.ending_lba, e.starting_lba + e.sectors() - 1);
    }

    #[test]
    fn unused_entries_are_zero_type_and_zero_length() {
        let e = GptEntry::default();
        assert!(!e.is_used());
        assert_eq!(e.sectors(), 0);
        assert_eq!(e.to_bytes(), [0u8; 128]);
    }

    #[test]
    fn overlap_is_inclusive_at_both_ends() {
        let e = sample(); // 2048..=1050623
        assert!(e.overlaps(2048, 2048), "touching the first sector overlaps");
        assert!(e.overlaps(1_050_623, 2_000_000), "touching the last overlaps");
        assert!(e.overlaps(0, u64::MAX));
        assert!(!e.overlaps(0, 2047), "ending just before must not overlap");
        assert!(!e.overlaps(1_050_624, 2_000_000), "starting just after must not");
    }

    #[test]
    fn unused_entries_never_overlap() {
        assert!(!GptEntry::default().overlaps(0, u64::MAX));
    }

    #[test]
    fn names_round_trip_through_utf16() {
        for n in ["", "Peios root", "ünïcødé ✓", "36 chars exactly xxxxxxxxxxxxxxxxxxx"] {
            let e = GptEntry {
                name: n.into(),
                ..sample()
            };
            if n.encode_utf16().count() <= NAME_UNITS {
                assert_eq!(GptEntry::parse(&e.to_bytes()).unwrap().name, n, "{n:?}");
            }
        }
    }

    #[test]
    fn a_too_long_name_is_rejected_not_truncated() {
        let long = "x".repeat(NAME_UNITS + 1);
        assert!(validate_name(&long).is_err());
        assert!(validate_name(&"x".repeat(NAME_UNITS)).is_ok());
        // Astral characters cost two units each, so 18 of them is the limit.
        assert!(validate_name(&"𐐷".repeat(18)).is_ok());
        assert!(validate_name(&"𐐷".repeat(19)).is_err());
    }
}
