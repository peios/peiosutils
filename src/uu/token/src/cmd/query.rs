// `token query <class>` — raw query of a single token-info class.
//
// Output is always structured: the human view is a hex dump and a
// class label; the JSON view carries both raw bytes and a typed
// decoding for classes we know how to decode.

use crate::cmd;
use crate::error::{Error, Result};
use crate::payload;
use crate::privs;
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::sid_render;
use crate::target::TargetSpec;
use peios::security::SidRef;
use peios::token::{Token, TokenAccess, TokenClass};
use peios_sys as sys;
use serde_json::json;

const KACS_TOKEN_QUERY: u32 = TokenAccess::QUERY.bits();

pub fn run(matches: &clap::ArgMatches, target: TargetSpec, mode: OutputMode) -> Result<()> {
    let class_name: &String = matches
        .get_one::<String>("class")
        .ok_or_else(|| Error::Usage("missing CLASS argument".into()))?;
    let class = parse_class(class_name)?;

    let tok = target.open(KACS_TOKEN_QUERY)?;
    let bytes = tok.query(class)?;
    let out = render_query(class_name, class, &bytes, &tok)?;
    cmd::emit(out, mode)
}

fn render_query(
    name: &str,
    class: TokenClass,
    bytes: &[u8],
    _tok: &Token,
) -> Result<CmdOutput> {
    let mut lines = Lines::new();
    let mut json = json!({
        "class": name,
        "class_id": class.0,
        "bytes_len": bytes.len(),
        "bytes_hex": hex_dump_inline(bytes),
    });
    lines.kv("class", name);
    lines.kv("class_id", format!("0x{:x}", class.0));
    lines.kv("bytes_len", bytes.len().to_string());

    // Best-effort typed decoding. `TokenClass` is a u32 newtype, so we match on
    // the raw KACS class id rather than the old `QueryClass` enum variants.
    let cls = class.0;
    if cls == sys::KACS_TOKEN_CLASS_USER
        || cls == sys::KACS_TOKEN_CLASS_OWNER
        || cls == sys::KACS_TOKEN_CLASS_PRIMARY_GROUP
        || cls == sys::KACS_TOKEN_CLASS_INTEGRITY_LEVEL
        || cls == sys::KACS_TOKEN_CLASS_APPCONTAINER_SID
        || cls == sys::KACS_TOKEN_CLASS_LOGON_SID
    {
        if !bytes.is_empty() {
            if let Some(sref) = SidRef::from_bytes(bytes) {
                let sid = sref.to_owned();
                lines.kv("decoded_sid", sid.to_string());
                json["decoded"] = sid_render::render_json(&sid);
            }
        }
    } else if cls == sys::KACS_TOKEN_CLASS_PRIVILEGES {
        let snap = privs::decode_privs_payload(bytes).map_err(Error::Decode)?;
        json["decoded"] = json!({
            "present": format!("0x{:016x}", snap.present),
            "enabled": format!("0x{:016x}", snap.enabled),
            "enabled_by_default": format!("0x{:016x}", snap.enabled_by_default),
            "used": format!("0x{:016x}", snap.used),
        });
        lines.detail(format!("present:            0x{:016x}", snap.present));
        lines.detail(format!("enabled:            0x{:016x}", snap.enabled));
        lines.detail(format!("enabled_by_default: 0x{:016x}", snap.enabled_by_default));
        lines.detail(format!("used:               0x{:016x}", snap.used));
    } else if cls == sys::KACS_TOKEN_CLASS_GROUPS
        || cls == sys::KACS_TOKEN_CLASS_RESTRICTED_SIDS
        || cls == sys::KACS_TOKEN_CLASS_DEVICE_GROUPS
        || cls == sys::KACS_TOKEN_CLASS_CAPABILITIES
    {
        let entries = payload::parse_sid_attrs_list(bytes).map_err(Error::Decode)?;
        let arr: Vec<_> = entries
            .iter()
            .map(|e| {
                let mut obj = sid_render::render_json(&e.sid);
                if let Some(o) = obj.as_object_mut() {
                    o.insert("attributes_raw".into(), e.attributes.into());
                }
                obj
            })
            .collect();
        json["decoded"] = json!({ "count": entries.len(), "entries": arr });
        lines.detail(format!("entries: {}", entries.len()));
    } else if cls == sys::KACS_TOKEN_CLASS_TYPE
        || cls == sys::KACS_TOKEN_CLASS_ELEVATION_TYPE
        || cls == sys::KACS_TOKEN_CLASS_IMPERSONATION_LEVEL
        || cls == sys::KACS_TOKEN_CLASS_SESSION_ID
        || cls == sys::KACS_TOKEN_CLASS_MANDATORY_POLICY
        || cls == sys::KACS_TOKEN_CLASS_LOGON_TYPE
    {
        let v = payload::parse_u32(bytes).map_err(Error::Decode)?;
        json["decoded"] = json!({ "u32": v });
        lines.detail(format!("u32: 0x{:x} ({})", v, v));
    } else if cls == sys::KACS_TOKEN_CLASS_SOURCE {
        let s = payload::parse_source(bytes).map_err(Error::Decode)?;
        json["decoded"] = json!({
            "name_bytes": s.name.to_vec(),
            "source_id": s.source_id,
        });
        lines.detail(format!("source_id: 0x{:x}", s.source_id));
    } else if cls == sys::KACS_TOKEN_CLASS_ORIGIN {
        let v = payload::parse_origin(bytes).map_err(Error::Decode)?;
        json["decoded"] = json!({ "origin": format!("0x{:016x}", v) });
        lines.detail(format!("origin: 0x{:016x}", v));
    } else if cls == sys::KACS_TOKEN_CLASS_STATISTICS {
        let s = payload::parse_statistics(bytes).map_err(Error::Decode)?;
        json["decoded"] = json!({ "fields": s.fields.to_vec() });
        for (i, f) in s.fields.iter().enumerate() {
            lines.detail(format!("field[{i}]: 0x{:016x}", f));
        }
    } else {
        // No typed decoding for this class yet. Hex dump only.
    }
    Ok(CmdOutput { human: lines, json })
}

fn hex_dump_inline(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn parse_class(name: &str) -> Result<TokenClass> {
    let raw = match name.to_ascii_lowercase().as_str() {
        "user" => sys::KACS_TOKEN_CLASS_USER,
        "groups" => sys::KACS_TOKEN_CLASS_GROUPS,
        "privileges" | "privs" => sys::KACS_TOKEN_CLASS_PRIVILEGES,
        "type" => sys::KACS_TOKEN_CLASS_TYPE,
        "integrity" | "integritylevel" | "integrity-level" => {
            sys::KACS_TOKEN_CLASS_INTEGRITY_LEVEL
        }
        "owner" => sys::KACS_TOKEN_CLASS_OWNER,
        "primarygroup" | "primary-group" => sys::KACS_TOKEN_CLASS_PRIMARY_GROUP,
        "sessionid" | "session-id" | "session" => sys::KACS_TOKEN_CLASS_SESSION_ID,
        "restrictedsids" | "restricted-sids" => sys::KACS_TOKEN_CLASS_RESTRICTED_SIDS,
        "source" => sys::KACS_TOKEN_CLASS_SOURCE,
        "statistics" | "stats" => sys::KACS_TOKEN_CLASS_STATISTICS,
        "origin" => sys::KACS_TOKEN_CLASS_ORIGIN,
        "elevationtype" | "elevation-type" | "elevation" => sys::KACS_TOKEN_CLASS_ELEVATION_TYPE,
        "devicegroups" | "device-groups" => sys::KACS_TOKEN_CLASS_DEVICE_GROUPS,
        "appcontainersid" | "appcontainer-sid" | "appcontainer" => {
            sys::KACS_TOKEN_CLASS_APPCONTAINER_SID
        }
        "capabilities" | "caps" => sys::KACS_TOKEN_CLASS_CAPABILITIES,
        "mandatorypolicy" | "mandatory-policy" => sys::KACS_TOKEN_CLASS_MANDATORY_POLICY,
        "logontype" | "logon-type" => sys::KACS_TOKEN_CLASS_LOGON_TYPE,
        "logonsid" | "logon-sid" => sys::KACS_TOKEN_CLASS_LOGON_SID,
        "defaultdacl" | "default-dacl" => sys::KACS_TOKEN_CLASS_DEFAULT_DACL,
        "impersonationlevel" | "impersonation-level" => sys::KACS_TOKEN_CLASS_IMPERSONATION_LEVEL,
        "userclaims" | "user-claims" => sys::KACS_TOKEN_CLASS_USER_CLAIMS,
        "deviceclaims" | "device-claims" => sys::KACS_TOKEN_CLASS_DEVICE_CLAIMS,
        "projectedsupplementarygids" | "projected-supplementary-gids" | "projected-gids" => {
            sys::KACS_TOKEN_CLASS_PROJECTED_SUPPLEMENTARY_GIDS
        }
        other => return Err(Error::Usage(format!("unknown query class: {other}"))),
    };
    Ok(TokenClass(raw))
}
