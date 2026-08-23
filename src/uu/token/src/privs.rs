// Privilege-name table helpers.
//
// This table was inlined from the old `libp_token::uapi::PRIVILEGES` because
// the `peios`/`peios-sys` crates shipped only typed `Privileges` flag constants
// and no name table. That is no longer true — `peios::security::Privileges`
// now carries the names, derived from the ABI headers so no bit number is
// written out by hand — and this copy should be deleted in favour of it. It
// cannot be yet: this crate pins `peios` to git tag v0.2.0, which predates the
// table, so the switch waits on that pin moving.
//
// Until then, treat this as a mirror that is known to drift, and note what the
// drift already cost: `SeBackup` (17), `SeRemoteShutdown` (24) and
// `SeSystemProfile` (11) were simply absent. Because `entries()` filtered *by
// this table*, a token holding any of them did not show them — `token show`
// under-reported a real token by two privileges with no indication anything
// was missing, which read as a policy failure rather than a display one. That
// is why `entries()` now walks the mask rather than the table.

/// (bit_index, name) for every named KACS privilege.
///
/// Bit numbers are the ABI's, from `pkm/uapi/pkm/token.h`.
const PRIVILEGES: &[(u32, &str)] = &[
    (2, "SeCreateToken"),
    (3, "SeAssignPrimaryToken"),
    (4, "SeLockMemory"),
    (5, "SeIncreaseQuota"),
    (7, "SeTcb"),
    (8, "SeSecurity"),
    (10, "SeLoadDriver"),
    (11, "SeSystemProfile"),
    (12, "SeSystemTime"),
    (13, "SeProfileSingleProcess"),
    (14, "SeIncreaseBasePriority"),
    (17, "SeBackup"),
    (18, "SeRestore"),
    (19, "SeShutdown"),
    (20, "SeDebug"),
    (21, "SeAudit"),
    (23, "SeChangeNotify"),
    (24, "SeRemoteShutdown"),
    (29, "SeImpersonate"),
    (35, "SeCreateSymbolicLink"),
    (63, "SeBindPrivilegedPort"),
];

/// Name → bit index. Case-insensitive.
pub fn bit_for_name(name: &str) -> Option<u32> {
    PRIVILEGES
        .iter()
        .find(|(_, n)| n.eq_ignore_ascii_case(name))
        .map(|(b, _)| *b)
}

/// Bit index → name (the `SeXxxPrivilege` form).
pub fn name_for_bit(bit: u32) -> Option<&'static str> {
    PRIVILEGES.iter().find(|(b, _)| *b == bit).map(|(_, n)| *n)
}

/// All privilege (bit, name) tuples.
pub fn all() -> impl Iterator<Item = (u32, &'static str)> {
    PRIVILEGES.iter().copied()
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
        let mut bits: Vec<u32> = PRIVILEGES.iter().map(|(b, _)| *b).collect();
        bits.sort_unstable();
        let before = bits.len();
        bits.dedup();
        assert_eq!(before, bits.len(), "two entries share a bit");

        let mut names: Vec<String> =
            PRIVILEGES.iter().map(|(_, n)| n.to_ascii_lowercase()).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "two entries share a name");
    }
}
