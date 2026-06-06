// Parsers for KACS token-query payload classes that libp-token returns
// as raw bytes. The wire formats are documented kernel-side in
// `pkm/kacs/token_runtime.rs::write_query`. We mirror them here for the
// read-only inspection surface.

use libp_token::uapi::{Sid, SidRef};

/// Generic "list of (SID, attributes)" payload. Used for:
///   - TOKEN_CLASS_GROUPS
///   - TOKEN_CLASS_RESTRICTED_SIDS
///   - TOKEN_CLASS_DEVICE_GROUPS
///   - TOKEN_CLASS_CAPABILITIES
///
/// Wire format: `u32 count` then `count * { u32 sid_len; sid_bytes; u32 attrs }`.
#[derive(Debug, Clone)]
pub struct SidAndAttrsEntry {
    pub sid: Sid,
    pub attributes: u32,
}

pub fn parse_sid_attrs_list(buf: &[u8]) -> Result<Vec<SidAndAttrsEntry>, String> {
    let mut cur = buf;
    let count = read_u32(&mut cur)?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let sid_len = read_u32(&mut cur)? as usize;
        if cur.len() < sid_len {
            return Err(format!(
                "sid+attrs list: entry {i} sid truncated (need {sid_len} bytes, have {})",
                cur.len()
            ));
        }
        let (sref, _rest) = SidRef::parse(&cur[..sid_len])
            .map_err(|e| format!("sid+attrs list: entry {i} parse: {e:?}"))?;
        let sid = sref.to_owned();
        cur = &cur[sid_len..];
        let attributes = read_u32(&mut cur)?;
        out.push(SidAndAttrsEntry { sid, attributes });
    }
    Ok(out)
}

/// TOKEN_CLASS_SOURCE payload: 8-byte source name + u64 source_id.
#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub name: [u8; 8],
    pub source_id: u64,
}

pub fn parse_source(buf: &[u8]) -> Result<SourceInfo, String> {
    if buf.len() < 16 {
        return Err(format!("source: need 16 bytes, got {}", buf.len()));
    }
    let mut name = [0u8; 8];
    name.copy_from_slice(&buf[..8]);
    let source_id = u64::from_le_bytes([
        buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
    ]);
    Ok(SourceInfo { name, source_id })
}

/// TOKEN_CLASS_ORIGIN payload: u64 origin LUID.
pub fn parse_origin(buf: &[u8]) -> Result<u64, String> {
    if buf.len() < 8 {
        return Err(format!("origin: need 8 bytes, got {}", buf.len()));
    }
    Ok(u64::from_le_bytes([
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
    ]))
}

/// TOKEN_CLASS_STATISTICS payload: 40 bytes. Shape mirrors NT's
/// TOKEN_STATISTICS — five u64 fields. We surface them by name on
/// best-effort terms; the exact kernel layout may grow.
#[derive(Debug, Clone, Copy)]
pub struct Statistics {
    pub fields: [u64; 5],
}

pub fn parse_statistics(buf: &[u8]) -> Result<Statistics, String> {
    if buf.len() < 40 {
        return Err(format!("statistics: need 40 bytes, got {}", buf.len()));
    }
    let mut fields = [0u64; 5];
    for (i, slot) in fields.iter_mut().enumerate() {
        let off = i * 8;
        *slot = u64::from_le_bytes([
            buf[off],
            buf[off + 1],
            buf[off + 2],
            buf[off + 3],
            buf[off + 4],
            buf[off + 5],
            buf[off + 6],
            buf[off + 7],
        ]);
    }
    Ok(Statistics { fields })
}

/// TOKEN_CLASS_LOGON_TYPE / MANDATORY_POLICY / ELEVATION_TYPE etc. — u32.
pub fn parse_u32(buf: &[u8]) -> Result<u32, String> {
    if buf.len() < 4 {
        return Err(format!("u32 payload: need 4 bytes, got {}", buf.len()));
    }
    Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]))
}

// ---------------------------------------------------------------------------
// Internal helpers.
// ---------------------------------------------------------------------------

fn read_u32(cur: &mut &[u8]) -> Result<u32, String> {
    if cur.len() < 4 {
        return Err(format!("truncated: need 4 bytes for u32, got {}", cur.len()));
    }
    let v = u32::from_le_bytes([cur[0], cur[1], cur[2], cur[3]]);
    *cur = &cur[4..];
    Ok(v)
}

// ---------------------------------------------------------------------------
// Group attribute bits — mirrors NT-style flags as used by KACS.
// ---------------------------------------------------------------------------

pub const SE_GROUP_MANDATORY: u32 = 0x0000_0001;
pub const SE_GROUP_ENABLED_BY_DEFAULT: u32 = 0x0000_0002;
pub const SE_GROUP_ENABLED: u32 = 0x0000_0004;
pub const SE_GROUP_OWNER: u32 = 0x0000_0008;
pub const SE_GROUP_USE_FOR_DENY_ONLY: u32 = 0x0000_0010;
pub const SE_GROUP_INTEGRITY: u32 = 0x0000_0020;
pub const SE_GROUP_INTEGRITY_ENABLED: u32 = 0x0000_0040;
pub const SE_GROUP_RESOURCE: u32 = 0x2000_0000;
pub const SE_GROUP_LOGON_ID: u32 = 0xC000_0000;

/// Human-readable list of the attribute bits set on a group entry.
pub fn group_attrs_labels(attrs: u32) -> Vec<&'static str> {
    let mut out = Vec::new();
    if attrs & SE_GROUP_MANDATORY != 0 {
        out.push("mandatory");
    }
    if attrs & SE_GROUP_ENABLED_BY_DEFAULT != 0 {
        out.push("default");
    }
    if attrs & SE_GROUP_ENABLED != 0 {
        out.push("enabled");
    } else {
        out.push("disabled");
    }
    if attrs & SE_GROUP_OWNER != 0 {
        out.push("owner");
    }
    if attrs & SE_GROUP_USE_FOR_DENY_ONLY != 0 {
        out.push("deny-only");
    }
    if attrs & SE_GROUP_INTEGRITY != 0 {
        out.push("integrity");
    }
    if attrs & SE_GROUP_INTEGRITY_ENABLED != 0 {
        out.push("integrity-enabled");
    }
    if attrs & SE_GROUP_RESOURCE != 0 {
        out.push("resource");
    }
    if attrs & SE_GROUP_LOGON_ID == SE_GROUP_LOGON_ID {
        out.push("logon-id");
    }
    out
}
