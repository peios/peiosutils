// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! The GPT header (UEFI 2.10 §5.3.2), and the CRC discipline around it.
//!
//! Two CRC-32s protect a GPT, and both are CRC-32/ISO-HDLC — the same algorithm
//! `cksum -a crc32b` computes, which is why this reuses `crc_fast` rather than
//! carrying its own table.
//!
//! * `header_crc32` covers the first `header_size` bytes of the header **with
//!   its own field zeroed**. Forgetting to zero it is the classic bug: it
//!   produces a header that verifies only against itself.
//! * `entries_crc32` covers the whole partition entry array — all
//!   `num_entries * entry_size` bytes, including unused entries, which is why
//!   the array must be zero-filled rather than merely allocated.

use super::guid::Guid;
use crate::error::{PartError, Result};

pub const SIGNATURE: &[u8; 8] = b"EFI PART";
pub const REVISION: u32 = 0x0001_0000;
/// The spec fixes the header at 92 bytes; the rest of its sector is zero.
pub const HEADER_SIZE: u32 = 92;
/// Both values are spec minimums that every real-world tool also uses, so
/// matching them is what makes our tables interchangeable with theirs.
pub const ENTRY_SIZE: u32 = 128;
pub const NUM_ENTRIES: u32 = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GptHeader {
    pub my_lba: u64,
    pub alternate_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub disk_guid: Guid,
    pub entry_array_lba: u64,
    pub num_entries: u32,
    pub entry_size: u32,
    pub entries_crc32: u32,
}

impl GptHeader {
    /// Serialise into a whole sector: 92 bytes of header, zero to the end.
    ///
    /// The header CRC is computed here rather than being a field the caller can
    /// forget to refresh — every mutation therefore re-derives it by
    /// construction.
    pub fn to_sector(&self, sector_size: usize) -> Vec<u8> {
        let mut s = vec![0u8; sector_size];
        s[0..8].copy_from_slice(SIGNATURE);
        s[8..12].copy_from_slice(&REVISION.to_le_bytes());
        s[12..16].copy_from_slice(&HEADER_SIZE.to_le_bytes());
        // 16..20 is header_crc32 — left zero while it is being computed.
        // 20..24 is reserved and must stay zero.
        s[24..32].copy_from_slice(&self.my_lba.to_le_bytes());
        s[32..40].copy_from_slice(&self.alternate_lba.to_le_bytes());
        s[40..48].copy_from_slice(&self.first_usable_lba.to_le_bytes());
        s[48..56].copy_from_slice(&self.last_usable_lba.to_le_bytes());
        s[56..72].copy_from_slice(self.disk_guid.as_bytes());
        s[72..80].copy_from_slice(&self.entry_array_lba.to_le_bytes());
        s[80..84].copy_from_slice(&self.num_entries.to_le_bytes());
        s[84..88].copy_from_slice(&self.entry_size.to_le_bytes());
        s[88..92].copy_from_slice(&self.entries_crc32.to_le_bytes());

        let crc = crc32(&s[..HEADER_SIZE as usize]);
        s[16..20].copy_from_slice(&crc.to_le_bytes());
        s
    }

    /// Parse and validate a header sector.
    ///
    /// Validation is deliberately strict: a header that fails here is reported
    /// as damaged rather than repaired silently. `part` is not a recovery tool,
    /// and quietly "fixing" a table nobody asked it to touch is how data is
    /// lost.
    pub fn parse(sector: &[u8]) -> Result<GptHeader> {
        if sector.len() < HEADER_SIZE as usize {
            return Err(PartError::Damaged("header sector is too short".into()));
        }
        if &sector[0..8] != SIGNATURE {
            return Err(PartError::NoGpt);
        }
        let header_size = u32::from_le_bytes(sector[12..16].try_into().unwrap());
        if !(HEADER_SIZE..=sector.len() as u32).contains(&header_size) {
            return Err(PartError::Damaged(format!(
                "header size {header_size} is out of range"
            )));
        }

        // Recompute over header_size bytes with the CRC field zeroed, exactly as
        // it was produced.
        let stored = u32::from_le_bytes(sector[16..20].try_into().unwrap());
        let mut check = sector[..header_size as usize].to_vec();
        check[16..20].fill(0);
        let actual = crc32(&check);
        if stored != actual {
            return Err(PartError::Damaged(format!(
                "header CRC mismatch (stored {stored:#010x}, computed {actual:#010x})"
            )));
        }

        let entry_size = u32::from_le_bytes(sector[84..88].try_into().unwrap());
        if entry_size < ENTRY_SIZE || entry_size % 8 != 0 {
            return Err(PartError::Damaged(format!(
                "implausible partition entry size {entry_size}"
            )));
        }

        Ok(GptHeader {
            my_lba: u64::from_le_bytes(sector[24..32].try_into().unwrap()),
            alternate_lba: u64::from_le_bytes(sector[32..40].try_into().unwrap()),
            first_usable_lba: u64::from_le_bytes(sector[40..48].try_into().unwrap()),
            last_usable_lba: u64::from_le_bytes(sector[48..56].try_into().unwrap()),
            disk_guid: Guid::from_bytes(&sector[56..72]).unwrap_or(Guid::ZERO),
            entry_array_lba: u64::from_le_bytes(sector[72..80].try_into().unwrap()),
            num_entries: u32::from_le_bytes(sector[80..84].try_into().unwrap()),
            entry_size,
            entries_crc32: u32::from_le_bytes(sector[88..92].try_into().unwrap()),
        })
    }
}

/// CRC-32/ISO-HDLC, the algorithm GPT specifies for both of its checksums.
pub fn crc32(data: &[u8]) -> u32 {
    let mut d = crc_fast::Digest::new(crc_fast::CrcAlgorithm::Crc32IsoHdlc);
    d.update(data);
    d.finalize() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the algorithm choice itself. `crc32("123456789")` is the published
    /// check value for CRC-32/ISO-HDLC; if a future crc_fast bump changed what
    /// this variant means, every table we write would become unreadable and
    /// this is the test that would say so.
    #[test]
    fn crc32_matches_the_published_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn crc32_of_nothing_is_zero() {
        assert_eq!(crc32(b""), 0);
    }

    fn sample() -> GptHeader {
        GptHeader {
            my_lba: 1,
            alternate_lba: 16_777_215,
            first_usable_lba: 34,
            last_usable_lba: 16_777_182,
            disk_guid: Guid::parse("01020304-0506-0708-090A-0B0C0D0E0F10").unwrap(),
            entry_array_lba: 2,
            num_entries: NUM_ENTRIES,
            entry_size: ENTRY_SIZE,
            entries_crc32: 0xDEAD_BEEF,
        }
    }

    #[test]
    fn round_trips() {
        let h = sample();
        assert_eq!(GptHeader::parse(&h.to_sector(512)).unwrap(), h);
    }

    #[test]
    fn fields_land_at_their_spec_offsets() {
        let s = sample().to_sector(512);
        assert_eq!(&s[0..8], SIGNATURE);
        assert_eq!(u32::from_le_bytes(s[8..12].try_into().unwrap()), REVISION);
        assert_eq!(u32::from_le_bytes(s[12..16].try_into().unwrap()), HEADER_SIZE);
        assert_eq!(u64::from_le_bytes(s[24..32].try_into().unwrap()), 1);
        assert_eq!(u64::from_le_bytes(s[32..40].try_into().unwrap()), 16_777_215);
        assert_eq!(u32::from_le_bytes(s[88..92].try_into().unwrap()), 0xDEAD_BEEF);
        // Reserved must be zero, and so must everything past the 92-byte header.
        assert_eq!(&s[20..24], &[0, 0, 0, 0]);
        assert!(s[92..].iter().all(|&b| b == 0));
        assert_eq!(s.len(), 512);
    }

    /// The header CRC must be computed with its own field zeroed. If it were
    /// not, the stored value would depend on itself and this round-trip would
    /// be the only thing that ever agreed with it.
    #[test]
    fn header_crc_is_computed_over_a_zeroed_crc_field() {
        let s = sample().to_sector(512);
        let stored = u32::from_le_bytes(s[16..20].try_into().unwrap());
        let mut zeroed = s[..HEADER_SIZE as usize].to_vec();
        zeroed[16..20].fill(0);
        assert_eq!(stored, crc32(&zeroed));
        assert_ne!(stored, 0);
    }

    #[test]
    fn a_flipped_bit_is_caught() {
        let mut s = sample().to_sector(512);
        s[40] ^= 0x01; // first_usable_lba
        match GptHeader::parse(&s) {
            Err(PartError::Damaged(m)) => assert!(m.contains("CRC mismatch"), "{m}"),
            other => panic!("expected a CRC failure, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_signature_is_not_gpt_rather_than_damage() {
        let mut s = sample().to_sector(512);
        s[0..8].copy_from_slice(b"NOTGPT!!");
        assert!(matches!(GptHeader::parse(&s), Err(PartError::NoGpt)));
    }

    #[test]
    fn works_at_4096_byte_sectors() {
        let h = sample();
        let s = h.to_sector(4096);
        assert_eq!(s.len(), 4096);
        assert!(s[92..].iter().all(|&b| b == 0));
        assert_eq!(GptHeader::parse(&s).unwrap(), h);
    }
}
