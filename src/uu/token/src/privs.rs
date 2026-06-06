// Privilege-name table helpers. The authoritative list lives in
// `libp_token::uapi::PRIVILEGES` ((bit_index, name) tuples); this
// module wraps it with lookup, render, and parse helpers.

use libp_token::uapi::PRIVILEGES;

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
    /// Iterate (bit, name, enabled, enabled_by_default, used) tuples
    /// over the privileges that are present in this snapshot.
    pub fn entries(&self) -> impl Iterator<Item = PrivEntry> + '_ {
        PRIVILEGES.iter().filter_map(|(bit, name)| {
            let mask = 1u64 << *bit;
            if self.present & mask == 0 {
                return None;
            }
            Some(PrivEntry {
                bit: *bit,
                name,
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
    pub name: &'static str,
    pub enabled: bool,
    pub enabled_by_default: bool,
    pub used: bool,
}
