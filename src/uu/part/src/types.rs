// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Partition type GUIDs, and the short names people actually type.
//!
//! The aliases are deliberately few. A long table of every type GUID in
//! circulation would be a catalogue to maintain — the same data problem this
//! tool avoids elsewhere — and `--type` accepts a raw GUID, so nothing is out
//! of reach for want of an alias.

use crate::gpt::guid::Guid;

/// EFI system partition. UEFI 2.10 §5.3.3.
pub const ESP: &str = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B";
/// "Linux filesystem data" — what mke2fs-formatted partitions carry.
pub const LINUX: &str = "0FC63DAF-8483-4772-8E79-3D69D8477DE4";
/// Linux swap.
pub const SWAP: &str = "0657FD6D-A4AB-43C4-84E5-0933C84B4F4F";
/// Microsoft basic data — the type a FAT or NTFS volume normally carries when
/// it is not an ESP.
pub const MSDATA: &str = "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7";

/// `(alias, guid, description)`, in the order `part --help` lists them.
pub const ALIASES: &[(&str, &str, &str)] = &[
    ("esp", ESP, "EFI system partition"),
    ("linux", LINUX, "Linux filesystem data"),
    ("swap", SWAP, "Linux swap"),
    ("msdata", MSDATA, "Microsoft basic data"),
];

/// Resolve `--type`: an alias, or a raw GUID in either case.
pub fn resolve(spec: &str) -> Option<Guid> {
    let lower = spec.to_ascii_lowercase();
    for (alias, guid, _) in ALIASES {
        if *alias == lower {
            return Guid::parse(guid);
        }
    }
    Guid::parse(spec)
}

/// The human description for a GUID, when there is one.
pub fn describe(g: &Guid) -> Option<&'static str> {
    lookup(g).map(|(_, _, desc)| desc)
}

/// The short alias for a GUID, when there is one.
///
/// This is what `part list` puts in its TYPE column: `esp` rather than
/// "EFI system partition", because the long form is usually identical to the
/// partition's NAME and a table with the same words twice reads as an error.
pub fn alias(g: &Guid) -> Option<&'static str> {
    lookup(g).map(|(alias, _, _)| alias)
}

fn lookup(g: &Guid) -> Option<(&'static str, &'static str, &'static str)> {
    let s = g.to_string();
    ALIASES
        .iter()
        .find(|(_, guid, _)| guid.eq_ignore_ascii_case(&s))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_resolve() {
        assert_eq!(resolve("esp"), Guid::parse(ESP));
        assert_eq!(resolve("ESP"), Guid::parse(ESP));
        assert_eq!(resolve("linux"), Guid::parse(LINUX));
        assert_eq!(resolve("swap"), Guid::parse(SWAP));
    }

    #[test]
    fn raw_guids_resolve_in_either_case() {
        assert_eq!(resolve(ESP), Guid::parse(ESP));
        assert_eq!(resolve(&ESP.to_lowercase()), Guid::parse(ESP));
    }

    #[test]
    fn nonsense_does_not_resolve() {
        assert!(resolve("").is_none());
        assert!(resolve("not-a-type").is_none());
        // Close to an alias but not one — must not fuzzy-match onto a type that
        // would send the partition somewhere unintended.
        assert!(resolve("esp2").is_none());
        assert!(resolve("linux-root").is_none());
    }

    #[test]
    fn every_alias_is_a_parseable_guid_and_round_trips() {
        for (alias, guid, _) in ALIASES {
            let g = Guid::parse(guid).unwrap_or_else(|| panic!("{alias} has a bad GUID"));
            assert_eq!(g.to_string(), *guid, "{alias} must be canonical uppercase");
            assert!(describe(&g).is_some(), "{alias} should describe");
            assert_eq!(super::alias(&g), Some(*alias));
        }
    }

    #[test]
    fn describe_is_none_for_an_unknown_type() {
        let unknown = Guid::parse("11111111-2222-3333-4444-555555555555").unwrap();
        assert!(describe(&unknown).is_none());
        assert!(super::alias(&unknown).is_none());
    }
}
