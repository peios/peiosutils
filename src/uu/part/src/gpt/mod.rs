// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! A GPT as a value: build it, mutate it, serialise it.
//!
//! Nothing here touches a device. The whole table is assembled in memory and
//! handed to the device layer as a list of (LBA, bytes) writes, which is what
//! makes every layout rule below testable against a `Vec<u8>` rather than
//! against a disk.
//!
//! # Layout
//!
//! ```text
//!   LBA 0                     protective MBR
//!   LBA 1                     primary header
//!   LBA 2 .. 2+E-1            primary entry array      (E = ceil(16384/sector))
//!   first_usable = 2+E        ─┐
//!                              ├─ partitions live here
//!   last_usable = L-E-1       ─┘
//!   LBA L-E .. L-1            backup entry array
//!   LBA L                     backup header            (L = disk_sectors-1)
//! ```
//!
//! `E` is 32 at 512-byte sectors and 4 at 4096, so `first_usable` is 34 or 6 —
//! which is why none of this may hardcode 34.

pub mod entry;
pub mod guid;
pub mod header;
pub mod mbr;

use crate::error::{PartError, Result};
use entry::GptEntry;
use guid::Guid;
use header::{GptHeader, ENTRY_SIZE, NUM_ENTRIES};

/// Partition alignment, in bytes. 1 MiB is the universal convention: it divides
/// every erase block and stripe width in practice, and misalignment costs
/// read-modify-write cycles on flash for the life of the filesystem.
pub const ALIGN_BYTES: u64 = 1024 * 1024;

/// A parsed or freshly built GPT.
///
/// The array geometry and usable range are **carried, not recomputed**. A table
/// we create uses the conventional 128×128 array, but the spec sets 128 entries
/// as a *minimum* and real tables exceed it — a Peios ISO built by xorriso
/// declares 248, padding the array to a 64-sector boundary. Recomputing those
/// numbers from our own defaults would mean `part add` silently rewriting such
/// a table as a smaller one, discarding entries past 128 and moving
/// `first_usable` under partitions that already exist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gpt {
    pub sector_size: usize,
    pub disk_sectors: u64,
    pub disk_guid: Guid,
    /// Entry-array geometry, from the header when parsed.
    pub num_entries: u32,
    pub entry_size: u32,
    /// LBA of the primary entry array — 2 in practice, but stated by the header.
    pub entry_array_lba: u64,
    /// Usable range, from the header when parsed.
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    /// Always `num_entries` long; unused slots are `GptEntry::default()`.
    pub entries: Vec<GptEntry>,
}

/// An inclusive run of unallocated sectors, with `start` already aligned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreeExtent {
    pub start: u64,
    pub end: u64,
}

impl FreeExtent {
    pub fn sectors(&self) -> u64 {
        self.end.saturating_sub(self.start) + 1
    }
}

impl Gpt {
    /// Sectors occupied by one copy of the *default* entry array, for sizing a
    /// table we are about to create. Use [`Gpt::array_sectors`] for a table that
    /// already exists — its geometry is its own, not ours.
    pub fn entry_array_sectors(sector_size: usize) -> u64 {
        Self::array_sectors_for(NUM_ENTRIES, ENTRY_SIZE, sector_size)
    }

    /// Sectors occupied by one copy of *this* table's entry array.
    pub fn array_sectors(&self) -> u64 {
        Self::array_sectors_for(self.num_entries, self.entry_size, self.sector_size)
    }

    pub fn array_sectors_for(num_entries: u32, entry_size: u32, sector_size: usize) -> u64 {
        (u64::from(num_entries) * u64::from(entry_size)).div_ceil(sector_size as u64)
    }

    /// The smallest disk that can hold a GPT with room for one aligned
    /// partition. Below this, `create` refuses rather than producing a table
    /// with no usable space.
    pub fn minimum_sectors(sector_size: usize) -> u64 {
        let e = Self::entry_array_sectors(sector_size);
        // MBR + header + entries + at least one usable sector + entries + header
        3 + 2 * e + Self::align_sectors(sector_size)
    }

    /// Alignment in sectors, never zero even if a sector somehow exceeds 1 MiB.
    pub fn align_sectors(sector_size: usize) -> u64 {
        (ALIGN_BYTES / sector_size as u64).max(1)
    }

    pub fn first_usable(&self) -> u64 {
        self.first_usable_lba
    }

    pub fn last_usable(&self) -> u64 {
        self.last_usable_lba
    }

    pub fn last_lba(&self) -> u64 {
        self.disk_sectors - 1
    }

    pub fn backup_entries_lba(&self) -> u64 {
        self.last_lba() - self.array_sectors()
    }

    /// A fresh, empty table.
    pub fn create(disk_sectors: u64, sector_size: usize) -> Result<Gpt> {
        if !sector_size.is_power_of_two() || !(512..=65536).contains(&sector_size) {
            return Err(PartError::Usage(format!(
                "implausible logical sector size {sector_size}"
            )));
        }
        let min = Self::minimum_sectors(sector_size);
        if disk_sectors < min {
            return Err(PartError::Usage(format!(
                "disk is {disk_sectors} sectors; a GPT with usable space needs at least {min}"
            )));
        }
        let array = Self::entry_array_sectors(sector_size);
        Ok(Gpt {
            sector_size,
            disk_sectors,
            disk_guid: Guid::random(),
            num_entries: NUM_ENTRIES,
            entry_size: ENTRY_SIZE,
            entry_array_lba: 2,
            first_usable_lba: 2 + array,
            last_usable_lba: disk_sectors - 1 - array - 1,
            entries: vec![GptEntry::default(); NUM_ENTRIES as usize],
        })
    }

    /// Rebuild from the bytes the device layer read.
    ///
    /// `header_sector` is the primary header's sector and `entry_bytes` the
    /// whole primary entry array. The entry-array CRC is checked here, so a
    /// table whose header survived but whose entries did not is reported as
    /// damaged rather than parsed into plausible-looking nonsense.
    pub fn parse(
        disk_sectors: u64,
        sector_size: usize,
        header_sector: &[u8],
        entry_bytes: &[u8],
    ) -> Result<Gpt> {
        let h = GptHeader::parse(header_sector)?;

        let want = (h.num_entries as usize).saturating_mul(h.entry_size as usize);
        let array = entry_bytes
            .get(..want)
            .ok_or_else(|| PartError::Damaged("entry array is shorter than the header claims".into()))?;
        let actual = header::crc32(array);
        if actual != h.entries_crc32 {
            return Err(PartError::Damaged(format!(
                "partition entry CRC mismatch (stored {:#010x}, computed {actual:#010x})",
                h.entries_crc32
            )));
        }

        let mut entries = Vec::with_capacity(h.num_entries as usize);
        for i in 0..h.num_entries as usize {
            let off = i * h.entry_size as usize;
            entries.push(
                GptEntry::parse(&array[off..off + h.entry_size as usize])
                    .ok_or_else(|| PartError::Damaged(format!("entry {} is malformed", i + 1)))?,
            );
        }

        Ok(Gpt {
            sector_size,
            disk_sectors,
            disk_guid: h.disk_guid,
            num_entries: h.num_entries,
            entry_size: h.entry_size,
            entry_array_lba: h.entry_array_lba,
            first_usable_lba: h.first_usable_lba,
            last_usable_lba: h.last_usable_lba,
            entries,
        })
    }

    /// Serialise the entry array, zero-padded to its full size.
    ///
    /// The padding is not cosmetic: the CRC covers every byte of the array
    /// including unused entries, so a short or uninitialised buffer produces a
    /// checksum no other tool will agree with.
    fn entry_array(&self) -> Vec<u8> {
        let stride = self.entry_size as usize;
        let mut out = vec![0u8; self.num_entries as usize * stride];
        for (i, e) in self.entries.iter().take(self.num_entries as usize).enumerate() {
            // An entry_size larger than 128 is legal; the extra bytes are
            // reserved and stay zero. Write into the first 128 of each stride.
            out[i * stride..i * stride + 128].copy_from_slice(&e.to_bytes());
        }
        out
    }

    fn header_for(&self, primary: bool, entries_crc: u32) -> GptHeader {
        GptHeader {
            my_lba: if primary { 1 } else { self.last_lba() },
            alternate_lba: if primary { self.last_lba() } else { 1 },
            first_usable_lba: self.first_usable_lba,
            last_usable_lba: self.last_usable_lba,
            disk_guid: self.disk_guid,
            entry_array_lba: if primary {
                self.entry_array_lba
            } else {
                self.backup_entries_lba()
            },
            num_entries: self.num_entries,
            entry_size: self.entry_size,
            entries_crc32: entries_crc,
        }
    }

    /// The complete set of writes, **in the order they must be issued**.
    ///
    /// Entries precede headers so the disk never holds a valid header pointing
    /// at an entry array that was not written — if the machine dies midway, the
    /// table is either the old one or unreadable, never a confident lie about
    /// where partitions are. The protective MBR goes first because until a
    /// header exists it is the only thing standing between this disk and a tool
    /// that thinks the space is free.
    pub fn writes(&self) -> Vec<(u64, Vec<u8>)> {
        let array = self.entry_array();
        let crc = header::crc32(&array);
        let padded = {
            let want = (self.array_sectors() as usize) * self.sector_size;
            let mut v = array;
            v.resize(want, 0);
            v
        };
        vec![
            (0, mbr::protective(self.disk_sectors, self.sector_size)),
            (self.entry_array_lba, padded.clone()),
            (self.backup_entries_lba(), padded),
            (1, self.header_for(true, crc).to_sector(self.sector_size)),
            (
                self.last_lba(),
                self.header_for(false, crc).to_sector(self.sector_size),
            ),
        ]
    }

    /// Used partitions, as (1-based index, entry), in table order.
    pub fn partitions(&self) -> Vec<(usize, &GptEntry)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_used())
            .map(|(i, e)| (i + 1, e))
            .collect()
    }

    /// Unallocated runs, aligned and in ascending order.
    pub fn free_extents(&self) -> Vec<FreeExtent> {
        let align = Self::align_sectors(self.sector_size);
        let mut used: Vec<(u64, u64)> = self
            .entries
            .iter()
            .filter(|e| e.is_used())
            .map(|e| (e.starting_lba, e.ending_lba))
            .collect();
        used.sort_unstable();

        let mut out = Vec::new();
        let mut cursor = align_up(self.first_usable(), align);
        for (s, e) in used {
            // `cursor` may already be past this partition when two of them abut.
            if s > cursor {
                out.push(FreeExtent {
                    start: cursor,
                    end: s - 1,
                });
            }
            cursor = cursor.max(align_up(e.saturating_add(1), align));
        }
        if cursor <= self.last_usable() {
            out.push(FreeExtent {
                start: cursor,
                end: self.last_usable(),
            });
        }
        // An extent whose alignment consumed it entirely is not an extent.
        out.retain(|f| f.start <= f.end);
        out
    }

    /// Add a partition of `sectors` (or the largest free run when `None`).
    /// Returns the 1-based index it landed in.
    pub fn add(
        &mut self,
        sectors: Option<u64>,
        type_guid: Guid,
        name: &str,
        unique: Option<Guid>,
    ) -> Result<usize> {
        entry::validate_name(name)?;
        if type_guid.is_zero() {
            return Err(PartError::Usage(
                "a partition type GUID of all zeroes means 'unused'".into(),
            ));
        }
        let slot = self
            .entries
            .iter()
            .position(|e| !e.is_used())
            .ok_or_else(|| PartError::Usage(format!("all {NUM_ENTRIES} partition slots are in use")))?;

        let extents = self.free_extents();
        let chosen = match sectors {
            Some(n) if n == 0 => return Err(PartError::Usage("size must be non-zero".into())),
            Some(n) => extents
                .iter()
                .find(|f| f.sectors() >= n)
                .map(|f| FreeExtent {
                    start: f.start,
                    end: f.start + n - 1,
                })
                .ok_or_else(|| {
                    let biggest = extents.iter().map(|f| f.sectors()).max().unwrap_or(0);
                    PartError::NoSpace {
                        wanted: n,
                        largest: biggest,
                    }
                })?,
            None => *extents
                .iter()
                .max_by_key(|f| f.sectors())
                .ok_or(PartError::NoSpace {
                    wanted: 0,
                    largest: 0,
                })?,
        };

        self.entries[slot] = GptEntry {
            type_guid,
            unique_guid: unique.unwrap_or_else(Guid::random),
            starting_lba: chosen.start,
            ending_lba: chosen.end,
            attributes: 0,
            name: name.to_string(),
        };
        Ok(slot + 1)
    }

    /// Clear a partition by 1-based index.
    pub fn remove(&mut self, index: usize) -> Result<()> {
        let slot = index
            .checked_sub(1)
            .filter(|s| *s < self.entries.len())
            .ok_or_else(|| PartError::Usage(format!("no partition {index}")))?;
        if !self.entries[slot].is_used() {
            return Err(PartError::Usage(format!("partition {index} is not in use")));
        }
        self.entries[slot] = GptEntry::default();
        Ok(())
    }

    /// Structural problems a caller should be told about. Empty means healthy.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let align = Self::align_sectors(self.sector_size);
        let parts = self.partitions();

        for (i, e) in &parts {
            if e.starting_lba < self.first_usable() || e.ending_lba > self.last_usable() {
                out.push(format!(
                    "partition {i} ({}..{}) lies outside the usable range {}..{}",
                    e.starting_lba,
                    e.ending_lba,
                    self.first_usable(),
                    self.last_usable()
                ));
            }
            if e.ending_lba < e.starting_lba {
                out.push(format!("partition {i} ends before it starts"));
            }
            if e.starting_lba % align != 0 {
                out.push(format!(
                    "partition {i} starts at {}, which is not {}-sector aligned",
                    e.starting_lba, align
                ));
            }
        }
        for (a, (i, x)) in parts.iter().enumerate() {
            for (j, y) in parts.iter().skip(a + 1) {
                if x.overlaps(y.starting_lba, y.ending_lba) {
                    out.push(format!("partitions {i} and {j} overlap"));
                }
            }
        }
        out
    }
}

fn align_up(v: u64, align: u64) -> u64 {
    v.div_ceil(align) * align
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISK: u64 = 16_777_216; // 8 GiB at 512 bytes

    fn esp() -> Guid {
        Guid::parse("C12A7328-F81F-11D2-BA4B-00A0C93EC93B").unwrap()
    }
    fn linux() -> Guid {
        Guid::parse("0FC63DAF-8483-4772-8E79-3D69D8477DE4").unwrap()
    }

    #[test]
    fn layout_at_512_matches_the_universal_values() {
        let g = Gpt::create(DISK, 512).unwrap();
        assert_eq!(Gpt::entry_array_sectors(512), 32);
        assert_eq!(g.first_usable(), 34);
        assert_eq!(g.last_lba(), 16_777_215);
        assert_eq!(g.last_usable(), 16_777_182);
        assert_eq!(g.backup_entries_lba(), 16_777_183);
        assert_eq!(Gpt::align_sectors(512), 2048);
    }

    /// The reason nothing may hardcode 34: at 4 KiB sectors the array is four
    /// sectors, not thirty-two, and first_usable is 6.
    #[test]
    fn layout_at_4096_is_not_the_512_layout() {
        let g = Gpt::create(2_097_152, 4096).unwrap();
        assert_eq!(Gpt::entry_array_sectors(4096), 4);
        assert_eq!(g.first_usable(), 6);
        assert_eq!(g.last_usable(), 2_097_151 - 5);
        assert_eq!(Gpt::align_sectors(4096), 256);
    }

    #[test]
    fn rejects_implausible_sector_sizes() {
        assert!(Gpt::create(DISK, 511).is_err());
        assert!(Gpt::create(DISK, 0).is_err());
        assert!(Gpt::create(DISK, 1 << 20).is_err());
    }

    #[test]
    fn rejects_a_disk_too_small_to_hold_a_usable_table() {
        assert!(Gpt::create(64, 512).is_err());
        assert!(Gpt::create(Gpt::minimum_sectors(512), 512).is_ok());
        assert!(Gpt::create(Gpt::minimum_sectors(512) - 1, 512).is_err());
    }

    #[test]
    fn a_fresh_table_is_empty_and_wholly_free() {
        let g = Gpt::create(DISK, 512).unwrap();
        assert!(g.partitions().is_empty());
        let f = g.free_extents();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].start, 2048, "the first extent starts 1 MiB in");
        assert_eq!(f[0].end, g.last_usable());
        assert!(g.problems().is_empty());
    }

    #[test]
    fn add_places_the_first_partition_at_the_alignment_boundary() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        let i = g.add(Some(1_048_576), esp(), "EFI system partition", None).unwrap();
        assert_eq!(i, 1);
        let e = &g.entries[0];
        assert_eq!(e.starting_lba, 2048);
        assert_eq!(e.ending_lba, 1_050_623);
        assert_eq!(e.sectors(), 1_048_576);
        assert!(g.problems().is_empty());
    }

    #[test]
    fn a_second_partition_starts_after_the_first_and_stays_aligned() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        g.add(Some(1_048_576), esp(), "esp", None).unwrap();
        let i = g.add(None, linux(), "Peios root", None).unwrap();
        assert_eq!(i, 2);
        let e = &g.entries[1];
        assert_eq!(e.starting_lba, 1_050_624);
        assert_eq!(e.starting_lba % 2048, 0);
        assert_eq!(e.ending_lba, g.last_usable());
        assert!(g.problems().is_empty());
    }

    /// `max` takes the largest free run, which is not always the last one.
    #[test]
    fn max_size_picks_the_largest_extent_not_the_final_one() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        g.add(Some(2048), esp(), "a", None).unwrap();
        g.add(Some(2048), linux(), "b", None).unwrap();
        // Free the middle one, leaving a small hole and a huge tail.
        g.remove(2).unwrap();
        let i = g.add(None, linux(), "big", None).unwrap();
        assert_eq!(i, 2, "reuses the freed slot");
        assert!(
            g.entries[1].sectors() > 1_000_000,
            "should have taken the tail, got {} sectors",
            g.entries[1].sectors()
        );
    }

    #[test]
    fn a_request_that_does_not_fit_reports_the_largest_available() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        match g.add(Some(DISK * 2), esp(), "huge", None) {
            Err(PartError::NoSpace { wanted, largest }) => {
                assert_eq!(wanted, DISK * 2);
                assert!(largest > 0 && largest < DISK);
            }
            other => panic!("expected NoSpace, got {other:?}"),
        }
    }

    #[test]
    fn partitions_never_overlap_however_they_are_added() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        for n in 1..=8 {
            g.add(Some(2048 * n), linux(), &format!("p{n}"), None).unwrap();
        }
        assert_eq!(g.partitions().len(), 8);
        assert!(g.problems().is_empty(), "{:?}", g.problems());
    }

    #[test]
    fn remove_frees_the_space_and_the_slot() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        g.add(Some(2048), esp(), "a", None).unwrap();
        assert_eq!(g.partitions().len(), 1);
        g.remove(1).unwrap();
        assert!(g.partitions().is_empty());
        assert_eq!(g.free_extents()[0].start, 2048);
        assert!(g.remove(1).is_err(), "removing twice must fail");
        assert!(g.remove(0).is_err());
        assert!(g.remove(999).is_err());
    }

    #[test]
    fn a_zero_type_guid_is_refused_because_it_means_unused() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        assert!(g.add(Some(2048), Guid::ZERO, "x", None).is_err());
    }

    #[test]
    fn round_trips_through_serialisation() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        g.add(Some(1_048_576), esp(), "EFI system partition", None).unwrap();
        g.add(None, linux(), "Peios root", None).unwrap();

        let w = g.writes();
        let hdr = &w.iter().find(|(l, _)| *l == 1).unwrap().1;
        let ents = &w.iter().find(|(l, _)| *l == 2).unwrap().1;
        let back = Gpt::parse(DISK, 512, hdr, ents).unwrap();
        assert_eq!(back, g);
    }

    /// Entries must be written before the headers that vouch for them.
    #[test]
    fn writes_put_both_entry_copies_before_either_header() {
        let g = Gpt::create(DISK, 512).unwrap();
        let w = g.writes();
        let lbas: Vec<u64> = w.iter().map(|(l, _)| *l).collect();
        let primary_entries = lbas.iter().position(|&l| l == 2).unwrap();
        let backup_entries = lbas.iter().position(|&l| l == g.backup_entries_lba()).unwrap();
        let primary_header = lbas.iter().position(|&l| l == 1).unwrap();
        let backup_header = lbas.iter().position(|&l| l == g.last_lba()).unwrap();
        assert!(primary_entries < primary_header);
        assert!(backup_entries < primary_header);
        assert!(primary_entries < backup_header);
        assert!(backup_entries < backup_header);
        assert_eq!(lbas[0], 0, "protective MBR goes down first");
    }

    #[test]
    fn both_headers_describe_each_other_and_share_one_entry_crc() {
        let g = Gpt::create(DISK, 512).unwrap();
        let w = g.writes();
        let p = GptHeader::parse(&w.iter().find(|(l, _)| *l == 1).unwrap().1).unwrap();
        let b = GptHeader::parse(&w.iter().find(|(l, _)| *l == g.last_lba()).unwrap().1).unwrap();

        assert_eq!(p.my_lba, 1);
        assert_eq!(p.alternate_lba, g.last_lba());
        assert_eq!(b.my_lba, g.last_lba());
        assert_eq!(b.alternate_lba, 1);
        assert_eq!(p.entry_array_lba, 2);
        assert_eq!(b.entry_array_lba, g.backup_entries_lba());
        assert_eq!(p.entries_crc32, b.entries_crc32);
        assert_eq!(p.disk_guid, b.disk_guid);
        assert_eq!(p.first_usable_lba, b.first_usable_lba);
        assert_eq!(p.last_usable_lba, b.last_usable_lba);
    }

    /// The entry-array CRC covers unused entries too, so an empty table has a
    /// specific non-zero checksum rather than whatever a short buffer produced.
    #[test]
    fn the_entry_crc_covers_the_whole_padded_array() {
        let g = Gpt::create(DISK, 512).unwrap();
        let w = g.writes();
        let ents = &w.iter().find(|(l, _)| *l == 2).unwrap().1;
        assert_eq!(ents.len(), 32 * 512, "one full array, padded to sectors");
        let p = GptHeader::parse(&w.iter().find(|(l, _)| *l == 1).unwrap().1).unwrap();
        assert_eq!(p.entries_crc32, header::crc32(&ents[..(128 * 128)]));
    }

    #[test]
    fn a_corrupted_entry_array_is_caught_on_parse() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        g.add(Some(2048), esp(), "a", None).unwrap();
        let w = g.writes();
        let hdr = w.iter().find(|(l, _)| *l == 1).unwrap().1.clone();
        let mut ents = w.iter().find(|(l, _)| *l == 2).unwrap().1.clone();
        ents[40] ^= 0xff;
        match Gpt::parse(DISK, 512, &hdr, &ents) {
            Err(PartError::Damaged(m)) => assert!(m.contains("entry CRC"), "{m}"),
            other => panic!("expected damage, got {other:?}"),
        }
    }

    #[test]
    fn problems_reports_overlap_and_misalignment() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        g.add(Some(4096), esp(), "a", None).unwrap();
        // Hand-craft an overlapping, misaligned neighbour.
        g.entries[1] = GptEntry {
            type_guid: linux(),
            unique_guid: Guid::random(),
            starting_lba: 3000,
            ending_lba: 9000,
            attributes: 0,
            name: "bad".into(),
        };
        let p = g.problems();
        assert!(p.iter().any(|m| m.contains("overlap")), "{p:?}");
        assert!(p.iter().any(|m| m.contains("aligned")), "{p:?}");
    }

    #[test]
    fn a_partition_past_the_usable_range_is_reported() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        g.entries[0] = GptEntry {
            type_guid: linux(),
            unique_guid: Guid::random(),
            starting_lba: 2048,
            ending_lba: g.last_lba(), // into the backup header
            attributes: 0,
            name: "over".into(),
        };
        assert!(g.problems().iter().any(|m| m.contains("outside the usable range")));
    }

    #[test]
    fn every_slot_can_be_filled_and_the_next_add_fails() {
        let mut g = Gpt::create(DISK, 512).unwrap();
        for _ in 0..NUM_ENTRIES {
            g.add(Some(2048), linux(), "p", None).unwrap();
        }
        assert_eq!(g.partitions().len(), NUM_ENTRIES as usize);
        assert!(g.add(Some(2048), linux(), "one too many", None).is_err());
    }
}
