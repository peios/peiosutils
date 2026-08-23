// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Clap surface and command dispatch for `part`.

use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::device::{ensure_not_mounted, ensure_whole_disk, Device};
use crate::error::{PartError, Result};
use crate::gpt::Gpt;
use crate::label::{self, DiskLabel, DiskState};
use crate::types;

const ABOUT: &str = "Manage disk partition tables (GPT).";

const AFTER_HELP: &str = "\
Sizes accept K, M, G, T (powers of 1024) or a bare sector count, and `max`
takes the largest free run.

Types accept an alias or a raw GUID:
  esp      EFI system partition
  linux    Linux filesystem data
  swap     Linux swap
  msdata   Microsoft basic data

part manages GPT only. On a disk carrying anything else it says what it found
and stops; --force replaces it, destroying every partition on the disk.";

pub fn build() -> Command {
    let device = || {
        Arg::new("device")
            .required(true)
            .value_parser(clap::value_parser!(PathBuf))
            .help("the whole disk (not a partition)")
    };
    let yes = || {
        Arg::new("yes")
            .long("yes")
            .action(ArgAction::SetTrue)
            .help("confirm a destructive operation")
    };
    let force = || {
        Arg::new("force")
            .long("force")
            .action(ArgAction::SetTrue)
            .help("also replace a partition table part did not create")
    };

    Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .about(ABOUT)
        .after_help(AFTER_HELP)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("list")
                .about("Show the partition table, or every disk when given none")
                .arg(
                    Arg::new("device")
                        .value_parser(clap::value_parser!(PathBuf))
                        .help("the whole disk; omit to list every disk on the system"),
                ),
        )
        .subcommand(
            Command::new("verify")
                .about("Check the table's structure and checksums")
                .arg(device()),
        )
        .subcommand(
            Command::new("create")
                .about("Write a fresh, empty GPT")
                .arg(device())
                .arg(yes())
                .arg(force()),
        )
        .subcommand(
            Command::new("add")
                .about("Add a partition to the first free extent that fits")
                .arg(device())
                .arg(
                    Arg::new("size")
                        .long("size")
                        .default_value("max")
                        .help("size (512M, 2G, 4096s) or `max`"),
                )
                .arg(
                    Arg::new("type")
                        .long("type")
                        .default_value("linux")
                        .help("type alias or GUID"),
                )
                .arg(Arg::new("name").long("name").default_value("").help("partition name"))
                .arg(yes())
                .arg(force()),
        )
        .subcommand(
            Command::new("del")
                .about("Remove a partition by index")
                .arg(device())
                .arg(
                    Arg::new("index")
                        .required(true)
                        .value_parser(clap::value_parser!(usize))
                        .help("1-based partition number"),
                )
                .arg(yes())
                .arg(force()),
        )
}

pub fn dispatch(m: &ArgMatches) -> Result<()> {
    match m.subcommand() {
        Some(("list", a)) => cmd_list(a),
        Some(("verify", a)) => cmd_verify(a),
        Some(("create", a)) => cmd_create(a),
        Some(("add", a)) => cmd_add(a),
        Some(("del", a)) => cmd_del(a),
        _ => Err(PartError::Usage("no subcommand".into())),
    }
}

/// Open a device for a read-only command. The whole-disk guard applies even
/// here: reading a GPT "from" a partition would report a table that is not
/// really there.
fn open_ro(a: &ArgMatches) -> Result<Device> {
    let path: &PathBuf = a.get_one("device").unwrap();
    ensure_whole_disk(path)?;
    Device::open(path, false)
}

/// Open for a destructive command, with every guard applied first.
fn open_rw(a: &ArgMatches) -> Result<Device> {
    let path: &PathBuf = a.get_one("device").unwrap();
    if !a.get_flag("yes") {
        return Err(PartError::Refused(format!(
            "{} would be modified; pass --yes to confirm",
            path.display()
        )));
    }
    ensure_whole_disk(path)?;
    ensure_not_mounted(path)?;
    Device::open(path, true)
}

/// `part list` with no device: every disk on the system, one line each, with
/// its partitions beneath it.
///
/// A disk that cannot be opened or probed is still listed — with what sysfs
/// knows and a `?` for the table. Dropping it would be the worst possible
/// behaviour for an inventory command: the disk you cannot read is exactly the
/// one you most want to be told about.
fn cmd_list_all() -> Result<()> {
    let disks = crate::device::enumerate_disks();
    if disks.is_empty() {
        println!("no block devices");
        return Ok(());
    }

    println!("{:<14} {:>8}  {}", "DEVICE", "SIZE", "CONTENTS");
    for d in &disks {
        // Probing is best-effort. Anything that fails degrades this one line,
        // never the listing.
        let opened = Device::open(&d.path, false);
        let unreadable = opened.as_ref().err().map(unreadable_reason);
        let state = opened.ok().and_then(|mut dev| label::probe(&mut dev).ok());

        let summary = match (&state, unreadable) {
            (Some(DiskState::Gpt(g)), _) => {
                let n = g.partitions().len();
                format!("gpt, {n} partition{}", if n == 1 { "" } else { "s" })
            }
            (Some(other), _) => other.describe(),
            (None, Some(why)) => why,
            (None, None) => "?  (cannot be probed)".to_string(),
        };
        println!("{:<14} {:>8}  {}", d.path.display(), human(d.size_bytes), summary);

        // Partition rows come from sysfs, so a disk carrying a label `part`
        // cannot manage still shows what the kernel found on it.
        let gpt = match &state {
            Some(DiskState::Gpt(g)) => Some(DiskLabel::partitions(&**g)),
            _ => None,
        };
        for (pname, index) in &d.partitions {
            // Type and name are separate columns, so names line up down the
            // listing instead of starting wherever the type happened to end.
            let detail = gpt
                .as_ref()
                .and_then(|ps| ps.iter().find(|p| p.index == *index))
                .map(|p| format!("{:<10} {}", p.type_name, p.name))
                .unwrap_or_default();
            let detail = detail.trim_end().to_string();
            let size = std::fs::read_to_string(format!("/sys/class/block/{pname}/size"))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .map(|s| human(s * 512))
                .unwrap_or_else(|| "?".into());
            println!("  {:<12} {:>8}  {}", pname, size, detail);
        }
    }
    Ok(())
}

/// Why a device could not be opened, in words rather than an errno.
///
/// `ENOMEDIUM` is worth separating: an empty optical drive is not a broken
/// disk, and "cannot read this device" invites someone to go looking for a
/// permissions problem that does not exist.
fn unreadable_reason(e: &PartError) -> String {
    let PartError::Io { source, .. } = e else {
        return "?  (cannot be probed)".to_string();
    };
    match source.raw_os_error() {
        Some(libc::ENOMEDIUM) => "(no medium)".to_string(),
        Some(libc::EACCES) | Some(libc::EPERM) => "?  (not permitted to read this device)".to_string(),
        Some(libc::EBUSY) => "?  (device is busy)".to_string(),
        _ => "?  (cannot read this device)".to_string(),
    }
}

fn cmd_list(a: &ArgMatches) -> Result<()> {
    if a.get_one::<PathBuf>("device").is_none() {
        return cmd_list_all();
    }
    let mut dev = open_ro(a)?;
    let state = label::probe(&mut dev)?;

    println!(
        "{}: {} sectors of {} bytes ({})",
        dev.path.display(),
        dev.geom.sectors,
        dev.geom.sector_size,
        human(dev.geom.sectors * dev.geom.sector_size as u64)
    );

    let DiskState::Gpt(g) = &state else {
        // The whole point of the module: say what was found, not what was
        // missing.
        println!("{}", state.describe());
        return Ok(());
    };

    println!("Label:     gpt");
    println!("Disk GUID: {}", g.disk_guid);
    println!("Usable:    {} .. {}", g.first_usable(), g.last_usable());
    println!();

    let parts = DiskLabel::partitions(&**g);
    if parts.is_empty() {
        println!("(no partitions)");
    } else {
        println!(
            "{:>3}  {:>12}  {:>12}  {:>10}  {:<38}  {}",
            "#", "START", "END", "SIZE", "TYPE", "NAME"
        );
        for p in &parts {
            println!(
                "{:>3}  {:>12}  {:>12}  {:>10}  {:<38}  {}",
                p.index,
                p.start,
                p.end,
                human(p.sectors * dev.geom.sector_size as u64),
                p.type_name,
                p.name
            );
        }
    }

    // "Free" means space a new partition could actually occupy, which is not
    // the same as unallocated sectors: the run below the first alignment
    // boundary can never hold one. Reporting raw unallocated space would
    // promise room that `add` would then refuse to use.
    let extents = g.free_extents();
    let free: u64 = extents.iter().map(|f| f.sectors()).sum();
    let largest = extents.iter().map(|f| f.sectors()).max().unwrap_or(0);
    println!();
    println!(
        "Free (aligned): {}  in {} extent(s), largest {}",
        human(free * dev.geom.sector_size as u64),
        extents.len(),
        human(largest * dev.geom.sector_size as u64)
    );
    Ok(())
}

fn cmd_verify(a: &ArgMatches) -> Result<()> {
    let mut dev = open_ro(a)?;
    let g = label::load_gpt(&mut dev)?;
    let problems = g.problems();
    if problems.is_empty() {
        println!("{}: GPT is structurally sound", dev.path.display());
        return Ok(());
    }
    for p in &problems {
        eprintln!("{}: {p}", dev.path.display());
    }
    Err(PartError::Damaged(format!(
        "{} problem(s) found",
        problems.len()
    )))
}

fn cmd_create(a: &ArgMatches) -> Result<()> {
    let mut dev = open_rw(a)?;
    label::probe(&mut dev)?.permit_destructive(a.get_flag("force"))?;

    let g = Gpt::create(dev.geom.sectors, dev.geom.sector_size)?;
    label::commit(&mut dev, &g)?;
    println!(
        "{}: wrote a new GPT ({})",
        dev.path.display(),
        g.disk_guid
    );
    Ok(())
}

fn cmd_add(a: &ArgMatches) -> Result<()> {
    let mut dev = open_rw(a)?;
    label::probe(&mut dev)?.permit_destructive(a.get_flag("force"))?;
    let mut g = label::load_gpt(&mut dev)?;

    let spec: &String = a.get_one("size").unwrap();
    let sectors = parse_size(spec, dev.geom.sector_size)?;
    let type_spec: &String = a.get_one("type").unwrap();
    let type_guid = types::resolve(type_spec)
        .ok_or_else(|| PartError::Usage(format!("unknown partition type {type_spec:?}")))?;
    let name: &String = a.get_one("name").unwrap();

    let index = g.add(sectors, type_guid, name, None)?;
    label::commit(&mut dev, &g)?;

    let e = &g.entries[index - 1];
    println!(
        "{}: partition {index} at {}..{} ({})",
        dev.path.display(),
        e.starting_lba,
        e.ending_lba,
        human(e.sectors() * dev.geom.sector_size as u64)
    );
    Ok(())
}

fn cmd_del(a: &ArgMatches) -> Result<()> {
    let mut dev = open_rw(a)?;
    label::probe(&mut dev)?.permit_destructive(a.get_flag("force"))?;
    let mut g = label::load_gpt(&mut dev)?;
    let index: usize = *a.get_one("index").unwrap();
    g.remove(index)?;
    label::commit(&mut dev, &g)?;
    println!("{}: removed partition {index}", dev.path.display());
    Ok(())
}

/// Parse a size into sectors. `max` is `None`, meaning "the largest free run".
///
/// Suffixes are powers of 1024, matching every other size a Peios user types.
/// A bare number is *sectors*, not bytes, and `s` says so explicitly — because
/// "2048" meaning bytes would silently create a partition a thousand times
/// smaller than intended.
pub fn parse_size(spec: &str, sector_size: usize) -> Result<Option<u64>> {
    let s = spec.trim();
    if s.eq_ignore_ascii_case("max") {
        return Ok(None);
    }
    let bad = || PartError::Usage(format!("cannot parse size {spec:?}"));
    let (digits, mult) = match s.chars().last().ok_or_else(bad)? {
        'K' | 'k' => (&s[..s.len() - 1], 1024),
        'M' | 'm' => (&s[..s.len() - 1], 1024 * 1024),
        'G' | 'g' => (&s[..s.len() - 1], 1024 * 1024 * 1024),
        'T' | 't' => (&s[..s.len() - 1], 1024u64.pow(4)),
        'S' | 's' => (&s[..s.len() - 1], 0), // 0 marks "already sectors"
        _ => (s, 0),
    };
    let n: u64 = digits.trim().parse().map_err(|_| bad())?;
    if n == 0 {
        return Err(PartError::Usage("size must be non-zero".into()));
    }
    Ok(Some(if mult == 0 {
        n
    } else {
        let bytes = n.checked_mul(mult).ok_or_else(bad)?;
        bytes.div_ceil(sector_size as u64)
    }))
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes}{}", UNITS[0])
    } else if v < 10.0 {
        format!("{v:.1}{}", UNITS[i])
    } else {
        format!("{v:.0}{}", UNITS[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_surface_is_well_formed() {
        build().debug_assert();
    }

    #[test]
    fn sizes_convert_to_sectors() {
        assert_eq!(parse_size("512M", 512).unwrap(), Some(1_048_576));
        assert_eq!(parse_size("1G", 512).unwrap(), Some(2_097_152));
        assert_eq!(parse_size("512M", 4096).unwrap(), Some(131_072));
        assert_eq!(parse_size("1K", 512).unwrap(), Some(2));
    }

    /// A bare number is sectors, not bytes. Reading it as bytes would make
    /// `--size 2048` a 2 KiB partition instead of a 1 MiB one.
    #[test]
    fn a_bare_number_is_sectors() {
        assert_eq!(parse_size("2048", 512).unwrap(), Some(2048));
        assert_eq!(parse_size("2048s", 512).unwrap(), Some(2048));
        assert_eq!(parse_size("2048S", 512).unwrap(), Some(2048));
        // And it does not depend on the sector size, unlike a byte figure.
        assert_eq!(parse_size("2048", 4096).unwrap(), Some(2048));
    }

    #[test]
    fn max_is_none() {
        assert_eq!(parse_size("max", 512).unwrap(), None);
        assert_eq!(parse_size("MAX", 512).unwrap(), None);
    }

    #[test]
    fn lowercase_suffixes_work_too() {
        assert_eq!(parse_size("512m", 512).unwrap(), parse_size("512M", 512).unwrap());
        assert_eq!(parse_size("2g", 512).unwrap(), parse_size("2G", 512).unwrap());
    }

    #[test]
    fn a_size_that_is_not_a_whole_number_of_sectors_rounds_up() {
        // 1000 bytes at 512 is 1.95 sectors; rounding down would silently lose
        // the tail of the request.
        assert_eq!(parse_size("1000", 512).unwrap(), Some(1000));
        assert_eq!(parse_size("1K", 700).unwrap(), Some(2));
    }

    #[test]
    fn nonsense_sizes_are_rejected() {
        for s in ["", "abc", "-1", "1X", "M", "0", "0M", "1.5G"] {
            assert!(parse_size(s, 512).is_err(), "should reject {s:?}");
        }
    }

    #[test]
    fn overflow_is_rejected_rather_than_wrapped() {
        assert!(parse_size("99999999999999999999T", 512).is_err());
        assert!(parse_size(&format!("{}T", u64::MAX), 512).is_err());
    }

    #[test]
    fn human_sizes_read_sensibly() {
        assert_eq!(human(512), "512B");
        assert_eq!(human(1024), "1.0K");
        assert_eq!(human(536_870_912), "512M");
        assert_eq!(human(8 * 1024 * 1024 * 1024), "8.0G");
    }
}
