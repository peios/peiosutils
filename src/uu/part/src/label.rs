// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! The seam between "what is on this disk" and "what `part` can do with it".
//!
//! Only GPT is implemented. The [`DiskLabel`] trait exists so that adding a
//! second format later is a new implementor rather than an edit to every call
//! site — and, more immediately, so the CLI is written against a label rather
//! than against GPT specifically.
//!
//! # Reporting what was found, not what was missing
//!
//! [`probe`] answers with a [`DiskState`], and the distinctions it draws are the
//! whole point of the module. "No GPT" is ambiguous between a blank disk and an
//! MBR disk holding somebody's data; the first is the ordinary case for
//! `create` and the second must stop and say so.
//!
//! Detection leans on libblkid for formats `part` does not parse, because
//! identifying arbitrary foreign tables is a catalogue that grows with every
//! util-linux release — the same data-versus-logic split that justifies writing
//! GPT ourselves. But libblkid is *not* trusted for "is this GPT healthy": it
//! answers `PTTYPE=gpt` on the strength of a protective MBR alone, so a disk
//! whose header is corrupt still looks like GPT to it. That judgement is made
//! here, from our own parse.

use uucore::blkid;

use crate::device::Device;
use crate::error::{PartError, Result};
use crate::gpt::{entry::GptEntry, mbr, Gpt};

/// One partition, as any label would describe it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartitionView {
    pub index: usize,
    pub start: u64,
    pub end: u64,
    pub sectors: u64,
    pub type_name: String,
    pub name: String,
    pub uuid: String,
}

/// What a partition table must be able to tell `part`.
pub trait DiskLabel {
    /// Short name for the format, as `part list` prints it.
    fn kind(&self) -> &'static str;
    fn partitions(&self) -> Vec<PartitionView>;
    /// Writes to commit, in the order they must be issued.
    fn writes(&self) -> Vec<(u64, Vec<u8>)>;
    /// Structural problems; empty means healthy.
    fn problems(&self) -> Vec<String>;
}

impl DiskLabel for Gpt {
    fn kind(&self) -> &'static str {
        "gpt"
    }

    fn partitions(&self) -> Vec<PartitionView> {
        Gpt::partitions(self)
            .into_iter()
            .map(|(index, e): (usize, &GptEntry)| PartitionView {
                index,
                start: e.starting_lba,
                end: e.ending_lba,
                sectors: e.sectors(),
                type_name: crate::types::alias(&e.type_guid)
                    .map(str::to_string)
                    .unwrap_or_else(|| e.type_guid.to_string()),
                name: e.name.clone(),
                uuid: e.unique_guid.to_string(),
            })
            .collect()
    }

    fn writes(&self) -> Vec<(u64, Vec<u8>)> {
        Gpt::writes(self)
    }

    fn problems(&self) -> Vec<String> {
        Gpt::problems(self)
    }
}

/// What was found on a disk.
#[derive(Debug)]
pub enum DiskState {
    /// A healthy GPT.
    Gpt(Box<Gpt>),
    /// GPT structures are present but wrong. Reported, never silently repaired.
    Damaged(String),
    /// A partition table `part` does not manage.
    Foreign { kind: String },
    /// A filesystem written straight to the disk, with no partition table at
    /// all. Rare, destructive to overwrite, and easy to do by accident with
    /// `mkfs /dev/vdb`, so it gets its own case rather than reading as blank.
    Filesystem { fstype: String },
    /// Nothing recognisable.
    Blank,
}

impl DiskState {
    /// One line describing the disk, for `part list` and for refusals.
    pub fn describe(&self) -> String {
        match self {
            Self::Gpt(_) => "GPT".to_string(),
            Self::Damaged(m) => format!("a damaged GPT ({m})"),
            Self::Foreign { kind } => format!("{} partition table", pretty_kind(kind)),
            Self::Filesystem { fstype } => {
                format!("a {fstype} filesystem written directly to the disk, with no partition table")
            }
            Self::Blank => "no partition table".to_string(),
        }
    }

    /// May a destructive operation proceed?
    ///
    /// `--force` is a second confirmation, distinct from `--yes`: `--yes` means
    /// "I mean this destructive operation", `--force` means "and I know it
    /// destroys a table `part` did not create". Requiring both is proportionate
    /// for the one tool whose mistakes are unrecoverable.
    pub fn permit_destructive(&self, force: bool) -> Result<()> {
        match self {
            Self::Gpt(_) | Self::Blank => Ok(()),
            _ if force => Ok(()),
            other => Err(PartError::Foreign(format!(
                "this disk carries {}, which part cannot manage; \
                 pass --force to replace it — every partition on it will be lost",
                other.describe()
            ))),
        }
    }
}

fn pretty_kind(kind: &str) -> String {
    match kind {
        "dos" => "an MBR (dos)".into(),
        "mac" => "an Apple Partition Map".into(),
        "bsd" => "a BSD disklabel".into(),
        "sun" => "a Sun disklabel".into(),
        "sgi" => "an SGI disklabel".into(),
        "atari" => "an Atari".into(),
        other => format!("a {other}"),
    }
}

/// Read a disk and decide what is on it.
pub fn probe(dev: &mut Device) -> Result<DiskState> {
    let sector0 = dev.read_sectors(0, 1)?;
    let header = dev.read_sectors(1, 1)?;

    // Our own parse is authoritative for GPT. Read the entry array the HEADER
    // describes, not the one we would have written: 128 entries is the spec's
    // minimum, not its maximum, and real tables exceed it — a Peios ISO built
    // by xorriso declares 248, so a fixed 32-sector read finds a short array
    // and misreports a perfectly good table as damaged.
    match crate::gpt::header::GptHeader::parse(&header) {
        Ok(h) => {
            let sectors =
                Gpt::array_sectors_for(h.num_entries, h.entry_size, dev.geom.sector_size);
            // Refuse to chase an array that could not fit on the disk rather
            // than attempting a huge read: a header claiming millions of
            // entries is corruption, not a table.
            if sectors == 0 || h.entry_array_lba + sectors > dev.geom.sectors {
                return Ok(DiskState::Damaged(format!(
                    "header describes a {}-entry array at LBA {}, which does not fit on this disk",
                    h.num_entries, h.entry_array_lba
                )));
            }
            let entries = dev.read_sectors(h.entry_array_lba, sectors)?;
            match Gpt::parse(dev.geom.sectors, dev.geom.sector_size, &header, &entries) {
                Ok(g) => return Ok(DiskState::Gpt(Box::new(g))),
                Err(PartError::Damaged(m)) => return Ok(DiskState::Damaged(m)),
                Err(PartError::NoGpt) => {}
                Err(e) => return Err(e),
            }
        }
        Err(PartError::Damaged(m)) => return Ok(DiskState::Damaged(m)),
        // No signature: not a GPT at all. Fall through to the other formats.
        Err(PartError::NoGpt) => {}
        Err(e) => return Err(e),
    }

    // No usable GPT header. A protective MBR without one is a damaged table,
    // not a blank disk — something wrote a GPT here and it did not survive.
    if mbr::is_protective(&sector0) && !mbr::is_real_mbr(&sector0) {
        return Ok(DiskState::Damaged(
            "a protective MBR is present but there is no valid GPT header".into(),
        ));
    }
    if mbr::is_real_mbr(&sector0) {
        return Ok(DiskState::Foreign { kind: "dos".into() });
    }

    // Anything else, ask libblkid — it knows the formats we deliberately do not.
    match blkid::probe_path(&dev.path) {
        Ok(info) => {
            if let Some(pt) = info.pttype.filter(|p| p != "gpt") {
                return Ok(DiskState::Foreign { kind: pt });
            }
            if let Some(fs) = info.fstype {
                return Ok(DiskState::Filesystem { fstype: fs });
            }
            Ok(DiskState::Blank)
        }
        // libblkid missing or unable to probe is not evidence of anything. Our
        // own checks above already ruled out GPT and MBR, so report blank
        // rather than inventing a foreign table — but note that `create` is the
        // only destructive path this reaches, and it would have proceeded on a
        // genuinely blank disk anyway.
        Err(_) => Ok(DiskState::Blank),
    }
}

/// Load a healthy GPT, or explain why there is not one.
pub fn load_gpt(dev: &mut Device) -> Result<Gpt> {
    match probe(dev)? {
        DiskState::Gpt(g) => Ok(*g),
        DiskState::Damaged(m) => Err(PartError::Damaged(m)),
        other => Err(PartError::Foreign(format!(
            "this disk carries {} — there is no GPT to modify",
            other.describe()
        ))),
    }
}

/// Commit a label to a device and make the kernel see it.
pub fn commit(dev: &mut Device, label: &dyn DiskLabel) -> Result<()> {
    for (lba, data) in label.writes() {
        dev.write_at(lba, &data)?;
    }
    // Durability before visibility: the kernel must not be asked to re-read a
    // table that is still sitting in the page cache.
    dev.sync()?;
    dev.reread_partition_table()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpt::guid::Guid;
    use std::path::PathBuf;

    fn image(bytes: u64) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("disk.img");
        std::fs::File::create(&p).unwrap().set_len(bytes).unwrap();
        (dir, p)
    }

    fn esp() -> Guid {
        Guid::parse(crate::types::ESP).unwrap()
    }

    #[test]
    fn a_blank_image_reports_blank_and_permits_creation() {
        let (_d, p) = image(64 * 1024 * 1024);
        let mut dev = Device::open(&p, false).unwrap();
        let st = probe(&mut dev).unwrap();
        assert!(matches!(st, DiskState::Blank), "{st:?}");
        assert!(st.permit_destructive(false).is_ok());
        assert_eq!(st.describe(), "no partition table");
    }

    #[test]
    fn a_written_gpt_reads_back_as_gpt() {
        let (_d, p) = image(64 * 1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        let mut g = Gpt::create(dev.geom.sectors, dev.geom.sector_size).unwrap();
        g.add(Some(2048), esp(), "EFI system partition", None).unwrap();
        commit(&mut dev, &g).unwrap();

        match probe(&mut dev).unwrap() {
            DiskState::Gpt(back) => {
                assert_eq!(back.partitions().len(), 1);
                assert_eq!(DiskLabel::partitions(&*back)[0].name, "EFI system partition");
                assert_eq!(back.disk_guid, g.disk_guid);
            }
            other => panic!("expected GPT, got {other:?}"),
        }
    }

    /// The row libblkid cannot supply: a protective MBR with a corrupt header
    /// still answers `PTTYPE=gpt`, so this judgement has to be ours.
    #[test]
    fn a_protective_mbr_with_a_broken_header_reports_damage() {
        let (_d, p) = image(64 * 1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        let g = Gpt::create(dev.geom.sectors, dev.geom.sector_size).unwrap();
        commit(&mut dev, &g).unwrap();

        // Corrupt the primary header, leaving the protective MBR intact.
        let mut h = dev.read_sectors(1, 1).unwrap();
        h[40] ^= 0xff;
        dev.write_at(1, &h).unwrap();
        dev.sync().unwrap();

        let st = probe(&mut dev).unwrap();
        assert!(matches!(st, DiskState::Damaged(_)), "{st:?}");
        assert!(st.permit_destructive(false).is_err(), "damage must stop us");
        assert!(st.permit_destructive(true).is_ok(), "--force is the way through");
    }

    #[test]
    fn a_real_mbr_is_foreign_and_refuses_without_force() {
        let (_d, p) = image(64 * 1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        let mut s = vec![0u8; 512];
        s[510] = 0x55;
        s[511] = 0xAA;
        s[446 + 4] = 0x83; // a Linux partition somebody cares about
        dev.write_at(0, &s).unwrap();
        dev.sync().unwrap();

        let st = probe(&mut dev).unwrap();
        match &st {
            DiskState::Foreign { kind } => assert_eq!(kind, "dos"),
            other => panic!("expected foreign, got {other:?}"),
        }
        let msg = st.permit_destructive(false).unwrap_err().to_string();
        assert!(msg.contains("MBR"), "{msg}");
        assert!(msg.contains("--force"), "the message must say the way through: {msg}");
        assert!(st.permit_destructive(true).is_ok());
    }

    #[test]
    fn a_hybrid_mbr_is_treated_as_a_real_one() {
        let (_d, p) = image(64 * 1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        let mut s = mbr::protective(dev.geom.sectors, 512);
        s[446 + 16 + 4] = 0x83;
        dev.write_at(0, &s).unwrap();
        dev.sync().unwrap();
        assert!(matches!(
            probe(&mut dev).unwrap(),
            DiskState::Foreign { .. }
        ));
    }

    #[test]
    fn load_gpt_refuses_a_foreign_disk_with_a_useful_message() {
        let (_d, p) = image(64 * 1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        let mut s = vec![0u8; 512];
        s[510] = 0x55;
        s[511] = 0xAA;
        s[446 + 4] = 0x83;
        dev.write_at(0, &s).unwrap();
        dev.sync().unwrap();
        let e = load_gpt(&mut dev).unwrap_err();
        assert_eq!(e.exit_code(), 3, "a refusal, not a failure");
        assert!(e.to_string().contains("no GPT to modify"), "{e}");
    }

    #[test]
    fn describe_names_each_foreign_label_readably() {
        for (kind, want) in [
            ("dos", "MBR"),
            ("mac", "Apple Partition Map"),
            ("bsd", "BSD disklabel"),
            ("sun", "Sun disklabel"),
        ] {
            let s = DiskState::Foreign { kind: kind.into() }.describe();
            assert!(s.contains(want), "{kind} -> {s}");
        }
    }

    #[test]
    fn a_bare_filesystem_is_reported_as_such() {
        let st = DiskState::Filesystem {
            fstype: "ext4".into(),
        };
        assert!(st.describe().contains("ext4"));
        assert!(st.describe().contains("no partition table"));
        assert!(st.permit_destructive(false).is_err());
    }

    #[test]
    fn commit_then_reload_survives_a_full_round_trip_with_several_partitions() {
        let (_d, p) = image(256 * 1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        let mut g = Gpt::create(dev.geom.sectors, dev.geom.sector_size).unwrap();
        g.add(Some(2048 * 8), esp(), "esp", None).unwrap();
        g.add(None, Guid::parse(crate::types::LINUX).unwrap(), "root", None)
            .unwrap();
        commit(&mut dev, &g).unwrap();

        let back = load_gpt(&mut dev).unwrap();
        assert_eq!(back, g);
        assert!(back.problems().is_empty());
    }

    /// Build a table with a non-default array geometry, the way real tools do.
    /// xorriso writes 248 entries on the Peios ISO, padding the array to a
    /// 64-sector boundary.
    fn wide_table(sectors: u64) -> Gpt {
        let mut g = Gpt::create(sectors, 512).unwrap();
        let arr = Gpt::array_sectors_for(248, 128, 512);
        g.num_entries = 248;
        g.entry_size = 128;
        g.first_usable_lba = 2 + arr;
        g.last_usable_lba = g.last_lba() - arr - 1;
        g.entries.resize(248, GptEntry::default());
        g
    }

    /// The regression this exists for: a fixed 32-sector read of the entry
    /// array reported the Peios ISO's own perfectly valid 248-entry GPT as
    /// "damaged (entry array is shorter than the header claims)". 128 entries
    /// is the spec's MINIMUM, not its maximum.
    #[test]
    fn a_table_with_more_than_128_entries_is_read_not_called_damaged() {
        let (_d, p) = image(64 * 1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        let mut g = wide_table(dev.geom.sectors);
        assert_eq!(g.array_sectors(), 62, "248 * 128 bytes is 62 sectors");
        assert_eq!(g.first_usable(), 64);
        g.add(Some(2048), esp(), "esp", None).unwrap();
        commit(&mut dev, &g).unwrap();

        match probe(&mut dev).unwrap() {
            DiskState::Gpt(back) => {
                assert_eq!(back.num_entries, 248, "geometry must survive the round trip");
                assert_eq!(back.first_usable(), 64);
                assert_eq!(back.partitions().len(), 1);
                assert_eq!(*back, g);
            }
            other => panic!("expected a healthy GPT, got {other:?}"),
        }
    }

    /// And editing such a table must not quietly shrink it back to 128 — that
    /// would discard entries past 128 and move first_usable under partitions
    /// that already exist.
    #[test]
    fn editing_a_wide_table_preserves_its_geometry() {
        let (_d, p) = image(64 * 1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        let g = wide_table(dev.geom.sectors);
        commit(&mut dev, &g).unwrap();

        let mut back = load_gpt(&mut dev).unwrap();
        back.add(Some(2048), esp(), "added later", None).unwrap();
        commit(&mut dev, &back).unwrap();

        let again = load_gpt(&mut dev).unwrap();
        assert_eq!(again.num_entries, 248, "still 248 after an edit");
        assert_eq!(again.first_usable(), 64);
        assert_eq!(again.partitions().len(), 1);
        assert!(again.problems().is_empty(), "{:?}", again.problems());
    }

    /// A header claiming an array that could not fit is corruption, and must be
    /// reported rather than turned into an enormous read.
    #[test]
    fn an_absurd_entry_count_is_damage_not_a_huge_read() {
        let (_d, p) = image(8 * 1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        let g = Gpt::create(dev.geom.sectors, 512).unwrap();
        commit(&mut dev, &g).unwrap();

        // Rewrite the header claiming 4 million entries, CRC recomputed so it
        // fails on plausibility rather than on the checksum.
        let mut h = crate::gpt::header::GptHeader::parse(&dev.read_sectors(1, 1).unwrap()).unwrap();
        h.num_entries = 4_000_000;
        dev.write_at(1, &h.to_sector(512)).unwrap();
        dev.sync().unwrap();

        match probe(&mut dev).unwrap() {
            DiskState::Damaged(m) => assert!(m.contains("does not fit"), "{m}"),
            other => panic!("expected damage, got {other:?}"),
        }
    }

    /// The backup copy must be a real copy, not an afterthought — a disk whose
    /// primary header is lost is still describable from the tail.
    #[test]
    fn the_backup_header_and_entries_are_written_too() {
        let (_d, p) = image(64 * 1024 * 1024);
        let mut dev = Device::open(&p, true).unwrap();
        let mut g = Gpt::create(dev.geom.sectors, dev.geom.sector_size).unwrap();
        g.add(Some(2048), esp(), "esp", None).unwrap();
        commit(&mut dev, &g).unwrap();

        let last = g.last_lba();
        let backup = dev.read_sectors(last, 1).unwrap();
        let hdr = crate::gpt::header::GptHeader::parse(&backup).unwrap();
        assert_eq!(hdr.my_lba, last);
        assert_eq!(hdr.alternate_lba, 1);

        let be = dev
            .read_sectors(g.backup_entries_lba(), Gpt::entry_array_sectors(512))
            .unwrap();
        let pe = dev.read_sectors(2, Gpt::entry_array_sectors(512)).unwrap();
        assert_eq!(be, pe, "both copies of the entry array must match");
    }
}
