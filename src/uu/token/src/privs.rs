// Privilege-name table helpers.
//
// This table used to be a hand-written copy of the old
// `libp_token::uapi::PRIVILEGES`, and it drifted, twice. First `SeBackup` (17),
// `SeRemoteShutdown` (24) and `SeSystemProfile` (11) were simply absent;
// because `entries()` filtered *by* this table, a token holding any of them did
// not show them, so `token show` under-reported a real token with no indication
// anything was missing, which read as a policy failure rather than a display
// one. That is why `entries()` now walks the mask rather than the table. Then
// `SeManageVolume` (28) went missing the same way and printed as
// `<privilege bit 28>` on a booted image (PEI-1077).
//
// So the table is no longer written here. It is derived from
// `peios::security::Privileges`, whose own bits come from the ABI headers, plus
// the handful of privileges the headers name that `Privileges` has no constant
// for yet — and those take their bits from `peios_sys` rather than from a
// literal. Nothing below writes a bit number by hand, which is the point: a
// name↔bit mapping typed out in two places drifts silently, and a wrong bit
// still *is* a bit.

use std::sync::OnceLock;

use peios::security::Privileges;

/// Privileges the ABI header names but `peios::security::Privileges` has no
/// constant for (PEI-186). Their bits still come from the header, through
/// `peios_sys`, so this list adds names and never bit numbers. Drop an entry
/// here when `Privileges` grows the matching constant.
const UNNAMED_BY_PEIOS_RS: &[(u64, &str)] = &[
    (
        peios_sys::KACS_SE_TAKE_OWNERSHIP_PRIVILEGE as u64,
        "SeTakeOwnership",
    ),
    (
        peios_sys::KACS_SE_SYSTEM_PROFILE_PRIVILEGE as u64,
        "SeSystemProfile",
    ),
    (peios_sys::KACS_SE_RELABEL_PRIVILEGE as u64, "SeRelabel"),
];

/// (bit_index, name) for every named KACS privilege, in bit order.
///
/// Names are the short form the applet displays — `peios-rs`'s canonical
/// `SeXxxPrivilege` with the `Privilege` suffix removed.
fn privileges() -> &'static [(u32, &'static str)] {
    static TABLE: OnceLock<Vec<(u32, &'static str)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let derived = Privileges::all_named().filter_map(|(name, privilege)| {
            let bits = privilege.bits();
            // A canonical name describes exactly one bit; anything else is not
            // a privilege this table can address by LUID.
            (bits.count_ones() == 1)
                .then(|| (bits.trailing_zeros(), strip_privilege_suffix(name)))
        });
        let extra = UNNAMED_BY_PEIOS_RS
            .iter()
            .map(|(mask, name)| (mask.trailing_zeros(), *name));

        let mut table: Vec<(u32, &'static str)> = derived.chain(extra).collect();
        table.sort_unstable_by_key(|(bit, _)| *bit);
        table
    })
}

/// Drop a trailing `Privilege`, matched case-insensitively, from a privilege
/// name. `SeDebugPrivilege` and `sedebugprivilege` both become the short form
/// the applet displays; a name that is only the suffix is left alone.
fn strip_privilege_suffix(name: &str) -> &str {
    const SUFFIX: &str = "Privilege";
    let Some(head_len) = name.len().checked_sub(SUFFIX.len()).filter(|n| *n > 0) else {
        return name;
    };
    match (name.get(..head_len), name.get(head_len..)) {
        (Some(head), Some(tail)) if tail.eq_ignore_ascii_case(SUFFIX) => head,
        _ => name,
    }
}

/// Name → bit index. Case-insensitive.
pub fn bit_for_name(name: &str) -> Option<u32> {
    // Both the short form the applet prints (`SeDebug`) and the canonical form
    // the rest of Peios writes (`SeDebugPrivilege`) are accepted, so an
    // operator can paste a name out of the documentation.
    let short = strip_privilege_suffix(name);
    privileges()
        .iter()
        .find(|(_, n)| n.eq_ignore_ascii_case(short))
        .map(|(b, _)| *b)
}

/// Bit index → name (the short `SeXxx` form).
pub fn name_for_bit(bit: u32) -> Option<&'static str> {
    privileges().iter().find(|(b, _)| *b == bit).map(|(_, n)| *n)
}

/// All privilege (bit, name) tuples.
pub fn all() -> impl Iterator<Item = (u32, &'static str)> {
    privileges().iter().copied()
}

/// Parse a LUID/name from a CLI token. Accepts:
///   - privilege name (e.g. `SeDebugPrivilege`, case-insensitive)
///   - decimal bit index (`23`)
///   - hex bit index (`0x17`)
pub fn parse_bit(s: &str) -> Result<u32, String> {
    if let Some(bit) = bit_for_name(s) {
        return Ok(bit);
    }
    let trimmed = s.trim();
    let parsed = if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16)
    } else {
        trimmed.parse::<u32>()
    };
    parsed.map_err(|_| format!("not a privilege name or LUID: `{s}`"))
}

/// Decode the 32-byte `TOKEN_CLASS_PRIVILEGES` payload as four u64
/// masks: (present, enabled, enabled_by_default, used).
pub fn decode_privs_payload(bytes: &[u8]) -> Result<PrivSnapshot, String> {
    if bytes.len() < 32 {
        return Err(format!(
            "privileges payload too short: {} bytes, need 32",
            bytes.len()
        ));
    }
    let read_u64 = |off: usize| {
        u64::from_le_bytes([
            bytes[off],
            bytes[off + 1],
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
            bytes[off + 6],
            bytes[off + 7],
        ])
    };
    Ok(PrivSnapshot {
        present: read_u64(0),
        enabled: read_u64(8),
        enabled_by_default: read_u64(16),
        used: read_u64(24),
    })
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PrivSnapshot {
    pub present: u64,
    pub enabled: u64,
    pub enabled_by_default: u64,
    pub used: u64,
}

impl PrivSnapshot {
    /// Every privilege present in this snapshot, in bit order.
    ///
    /// **Walks the mask, not the table.** A present bit this build has no name
    /// for is still reported, with `name: None`, because the alternative is
    /// what shipped before: a token silently displayed as holding fewer
    /// privileges than it does. A name table is always a build-time snapshot of
    /// a growing ABI, so "I do not recognise this" has to be sayable — the
    /// display exists to describe the token, and a token minted against a newer
    /// header is a thing that legitimately exists.
    pub fn entries(&self) -> impl Iterator<Item = PrivEntry> + '_ {
        (0..u64::BITS).filter_map(|bit| {
            let mask = 1u64 << bit;
            if self.present & mask == 0 {
                return None;
            }
            Some(PrivEntry {
                bit,
                name: name_for_bit(bit),
                enabled: self.enabled & mask != 0,
                enabled_by_default: self.enabled_by_default & mask != 0,
                used: self.used & mask != 0,
            })
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PrivEntry {
    pub bit: u32,
    /// `None` for a present bit this build cannot name.
    pub name: Option<&'static str>,
    pub enabled: bool,
    pub enabled_by_default: bool,
    pub used: bool,
}

impl PrivEntry {
    /// How to render this privilege.
    ///
    /// An unnameable bit shows as its number rather than as `?`: the number is
    /// what the operator can actually act on — `token adjust` accepts a bare
    /// LUID — and it is what makes a missing table entry diagnosable instead of
    /// merely visible.
    pub fn label(&self) -> String {
        match self.name {
            Some(name) => name.to_string(),
            None => format!("<privilege bit {}>", self.bit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The regression this file's history is about: a present bit with no table
    /// entry must still be reported.
    #[test]
    fn an_unnameable_present_bit_is_reported_rather_than_hidden() {
        let snap = PrivSnapshot {
            present: (1u64 << 23) | (1u64 << 40),
            enabled: 1u64 << 23,
            enabled_by_default: 0,
            used: 0,
        };
        let entries: Vec<_> = snap.entries().collect();
        assert_eq!(entries.len(), 2, "the unnamed bit must not be dropped");
        assert_eq!(entries[0].name, Some("SeChangeNotify"));
        assert_eq!(entries[1].name, None);
        assert_eq!(entries[1].label(), "<privilege bit 40>");
    }

    /// The two that were actually missing, and cost a boot to diagnose.
    #[test]
    fn backup_and_remote_shutdown_are_nameable() {
        assert_eq!(name_for_bit(17), Some("SeBackup"));
        assert_eq!(name_for_bit(24), Some("SeRemoteShutdown"));
        assert_eq!(bit_for_name("SeBackup"), Some(17));
        assert_eq!(bit_for_name("SeRemoteShutdown"), Some(24));
    }

    /// The exact token that exposed the gap: 13 privileges granted by policy,
    /// of which the old table could name only 11.
    #[test]
    fn the_administrator_token_reports_every_privilege_it_holds() {
        let snap = PrivSnapshot {
            present: 0x0000_0008_218e_7520,
            enabled: 0x0000_0008_218e_7520,
            enabled_by_default: 0x0000_0008_218e_7520,
            used: 0x0000_0000_0080_0000,
        };
        let entries: Vec<_> = snap.entries().collect();
        assert_eq!(entries.len(), 13);
        assert!(entries.iter().all(|e| e.name.is_some()), "all 13 must be nameable");
        assert!(entries.iter().any(|e| e.name == Some("SeBackup")));
        assert!(entries.iter().any(|e| e.name == Some("SeRemoteShutdown")));
    }

    #[test]
    fn entries_come_back_in_bit_order() {
        let snap = PrivSnapshot {
            present: (1u64 << 35) | (1u64 << 2) | (1u64 << 23),
            enabled: 0,
            enabled_by_default: 0,
            used: 0,
        };
        let bits: Vec<u32> = snap.entries().map(|e| e.bit).collect();
        assert_eq!(bits, [2, 23, 35]);
    }

    #[test]
    fn no_two_privileges_share_a_bit_or_a_name() {
        let mut bits: Vec<u32> = privileges().iter().map(|(b, _)| *b).collect();
        bits.sort_unstable();
        let before = bits.len();
        bits.dedup();
        assert_eq!(before, bits.len(), "two entries share a bit");

        let mut names: Vec<String> = privileges()
            .iter()
            .map(|(_, n)| n.to_ascii_lowercase())
            .collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "two entries share a name");
    }

    /// PEI-1077: bit 28 printed as `<privilege bit 28>` on a booted image
    /// because the hand-written table jumped from 24 to 29.
    #[test]
    fn manage_volume_is_nameable() {
        assert_eq!(name_for_bit(28), Some("SeManageVolume"));
        assert_eq!(bit_for_name("SeManageVolume"), Some(28));

        let snap = PrivSnapshot {
            present: peios_sys::KACS_SE_MANAGE_VOLUME_PRIVILEGE as u64,
            enabled: peios_sys::KACS_SE_MANAGE_VOLUME_PRIVILEGE as u64,
            enabled_by_default: 0,
            used: 0,
        };
        let entries: Vec<_> = snap.entries().collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label(), "SeManageVolume");
    }

    /// The table is derived, so every name `peios-rs` knows must be in it at
    /// the bit `peios-rs` gives it. This is the assertion that makes the two
    /// unable to drift: adding a privilege to `Privileges` adds it here.
    #[test]
    fn every_privilege_peios_rs_names_is_in_the_table() {
        for (canonical, privilege) in Privileges::all_named() {
            let bit = privilege.bits().trailing_zeros();
            let short = canonical.strip_suffix("Privilege").unwrap_or(canonical);
            assert_eq!(
                name_for_bit(bit),
                Some(short),
                "{canonical} (bit {bit}) is missing or misnamed"
            );
            assert_eq!(bit_for_name(canonical), Some(bit), "{canonical} by name");
        }
    }

    /// The privileges the ABI header names that `Privileges` has no constant
    /// for yet (PEI-186) are still nameable here.
    #[test]
    fn the_privileges_peios_rs_cannot_name_are_still_in_the_table() {
        assert_eq!(name_for_bit(9), Some("SeTakeOwnership"));
        assert_eq!(name_for_bit(11), Some("SeSystemProfile"));
        assert_eq!(name_for_bit(32), Some("SeRelabel"));
    }

    /// Both spellings parse: the short form the applet prints and the
    /// canonical form the specifications and the rest of Peios write.
    #[test]
    fn both_the_short_and_the_canonical_spelling_parse() {
        assert_eq!(parse_bit("SeDebug"), Ok(20));
        assert_eq!(parse_bit("SeDebugPrivilege"), Ok(20));
        assert_eq!(parse_bit("sedebugprivilege"), Ok(20));
        // The canonical spelling is `SeSystemtimePrivilege`; the table used to
        // print `SeSystemTime`, which nothing else in Peios wrote.
        assert_eq!(name_for_bit(12), Some("SeSystemtime"));
        assert_eq!(parse_bit("SeSystemTime"), Ok(12));
    }
}
