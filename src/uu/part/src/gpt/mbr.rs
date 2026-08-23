// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! The protective MBR (UEFI 2.10 §5.2.3).
//!
//! LBA 0 of a GPT disk holds a legacy MBR describing one partition of type
//! `0xEE` spanning the whole disk. Its entire purpose is defensive: a tool that
//! understands MBR and not GPT sees a disk that is fully occupied by something
//! it does not recognise, and declines to "helpfully" reclaim the free space.
//!
//! Which is why `PTTYPE=gpt` from libblkid is not proof of a healthy GPT — the
//! protective MBR alone is enough to earn that answer. Distinguishing a real
//! GPT from a lone protective MBR is `part`'s job, and [`is_protective`] plus a
//! header parse is how it does it.

/// The `0xEE` OS type that marks a protective MBR partition record.
pub const PROTECTIVE_TYPE: u8 = 0xEE;

/// Build the protective MBR sector.
pub fn protective(disk_sectors: u64, sector_size: usize) -> Vec<u8> {
    let mut s = vec![0u8; sector_size];

    // Boot code (0..440), disk signature (440..444) and 444..446 stay zero: we
    // are not a bootloader, and UEFI does not execute this.
    let rec = &mut s[446..462];
    rec[0] = 0x00; // not bootable
    // Starting CHS 0/0/2, the spec's fixed value for "LBA 1".
    rec[1] = 0x00;
    rec[2] = 0x02;
    rec[3] = 0x00;
    rec[4] = PROTECTIVE_TYPE;
    // Ending CHS: the spec says 0xFFFFFF when the disk is larger than CHS can
    // address, which is every disk this will ever run on.
    rec[5] = 0xFF;
    rec[6] = 0xFF;
    rec[7] = 0xFF;
    // Starting LBA is 1 — the GPT header sector, immediately after this one.
    rec[8..12].copy_from_slice(&1u32.to_le_bytes());
    // Size in sectors, saturating at 0xFFFFFFFF for disks beyond 2 TiB at 512
    // bytes. The spec mandates the clamp rather than a wrapped value.
    let size = u32::try_from(disk_sectors.saturating_sub(1)).unwrap_or(u32::MAX);
    rec[12..16].copy_from_slice(&size.to_le_bytes());

    // Records 2..4 (462..510) stay zero.
    s[510] = 0x55;
    s[511] = 0xAA;
    s
}

/// Does this sector look like a protective MBR — the boot signature plus a
/// `0xEE` partition record?
pub fn is_protective(sector: &[u8]) -> bool {
    if sector.len() < 512 || sector[510] != 0x55 || sector[511] != 0xAA {
        return false;
    }
    (0..4).any(|i| sector[446 + i * 16 + 4] == PROTECTIVE_TYPE)
}

/// Does this sector carry a *real* MBR — a boot signature and at least one
/// non-empty record that is not the protective marker?
///
/// This is the "somebody's data is on this disk" signal, and the reason
/// `create` refuses without `--force`.
pub fn is_real_mbr(sector: &[u8]) -> bool {
    if sector.len() < 512 || sector[510] != 0x55 || sector[511] != 0xAA {
        return false;
    }
    (0..4).any(|i| {
        let t = sector[446 + i * 16 + 4];
        t != 0x00 && t != PROTECTIVE_TYPE
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_the_boot_signature_and_one_protective_record() {
        let s = protective(16_777_216, 512);
        assert_eq!(s[510], 0x55);
        assert_eq!(s[511], 0xAA);
        assert_eq!(s[446 + 4], PROTECTIVE_TYPE);
        assert_eq!(u32::from_le_bytes(s[454..458].try_into().unwrap()), 1);
        assert!(is_protective(&s));
        assert!(!is_real_mbr(&s), "a protective MBR is not a real one");
    }

    /// Size is "sectors after LBA 0", so an 8 GiB disk of 16777216 sectors
    /// records 16777215.
    #[test]
    fn size_excludes_lba_zero() {
        let s = protective(16_777_216, 512);
        assert_eq!(
            u32::from_le_bytes(s[458..462].try_into().unwrap()),
            16_777_215
        );
    }

    /// Beyond 2 TiB at 512-byte sectors the field cannot hold the real size and
    /// the spec requires the clamp — not a wrap, which would describe a tiny
    /// disk and defeat the whole point of the record.
    #[test]
    fn size_clamps_rather_than_wrapping_on_huge_disks() {
        let s = protective(0x1_0000_0000, 512);
        assert_eq!(u32::from_le_bytes(s[458..462].try_into().unwrap()), u32::MAX);
        let s = protective(u64::MAX, 512);
        assert_eq!(u32::from_le_bytes(s[458..462].try_into().unwrap()), u32::MAX);
    }

    #[test]
    fn boot_code_and_the_other_three_records_are_zero() {
        let s = protective(16_777_216, 512);
        assert!(s[0..446].iter().all(|&b| b == 0), "no boot code");
        assert!(s[462..510].iter().all(|&b| b == 0), "records 2-4 empty");
    }

    #[test]
    fn fills_a_4096_byte_sector() {
        let s = protective(2_097_152, 4096);
        assert_eq!(s.len(), 4096);
        // The signature stays at 510/511 — it is an offset into the sector, not
        // a distance from its end.
        assert_eq!((s[510], s[511]), (0x55, 0xAA));
        assert!(s[512..].iter().all(|&b| b == 0));
    }

    #[test]
    fn a_real_mbr_is_told_apart_from_a_protective_one() {
        let mut s = vec![0u8; 512];
        s[510] = 0x55;
        s[511] = 0xAA;
        s[446 + 4] = 0x83; // Linux
        assert!(is_real_mbr(&s));
        assert!(!is_protective(&s));
    }

    #[test]
    fn a_blank_sector_is_neither() {
        let s = vec![0u8; 512];
        assert!(!is_protective(&s));
        assert!(!is_real_mbr(&s));
    }

    /// An MBR carrying both a protective record and a real one is a hybrid MBR.
    /// It must not be mistaken for a plain protective MBR, because the real
    /// record describes something a user cares about.
    #[test]
    fn a_hybrid_mbr_reports_as_real() {
        let mut s = protective(16_777_216, 512);
        s[446 + 16 + 4] = 0x83;
        assert!(is_real_mbr(&s));
    }
}
