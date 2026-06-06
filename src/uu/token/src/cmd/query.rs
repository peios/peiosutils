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
use libp_token::{QueryClass, Token};
use libp_token::uapi::SidRef;
use libp_token::uapi::KACS_TOKEN_QUERY;
use serde_json::json;

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
    class: QueryClass,
    bytes: &[u8],
    _tok: &Token,
) -> Result<CmdOutput> {
    let mut lines = Lines::new();
    let mut json = json!({
        "class": name,
        "class_id": class as u32,
        "bytes_len": bytes.len(),
        "bytes_hex": hex_dump_inline(bytes),
    });
    lines.kv("class", name);
    lines.kv("class_id", format!("0x{:x}", class as u32));
    lines.kv("bytes_len", bytes.len().to_string());

    // Best-effort typed decoding.
    match class {
        QueryClass::User
        | QueryClass::Owner
        | QueryClass::PrimaryGroup
        | QueryClass::IntegrityLevel
        | QueryClass::AppContainerSid
        | QueryClass::LogonSid => {
            if !bytes.is_empty() {
                if let Ok((sref, _)) = SidRef::parse(bytes) {
                    let sid = sref.to_owned();
                    lines.kv("decoded_sid", sid.to_string());
                    json["decoded"] = sid_render::render_json(&sid);
                }
            }
        }
        QueryClass::Privileges => {
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
        }
        QueryClass::Groups
        | QueryClass::RestrictedSids
        | QueryClass::DeviceGroups
        | QueryClass::Capabilities => {
            let entries =
                payload::parse_sid_attrs_list(bytes).map_err(Error::Decode)?;
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
        }
        QueryClass::Type
        | QueryClass::ElevationType
        | QueryClass::ImpersonationLevel
        | QueryClass::SessionId
        | QueryClass::MandatoryPolicy
        | QueryClass::LogonType => {
            let v = payload::parse_u32(bytes).map_err(Error::Decode)?;
            json["decoded"] = json!({ "u32": v });
            lines.detail(format!("u32: 0x{:x} ({})", v, v));
        }
        QueryClass::Source => {
            let s = payload::parse_source(bytes).map_err(Error::Decode)?;
            json["decoded"] = json!({
                "name_bytes": s.name.to_vec(),
                "source_id": s.source_id,
            });
            lines.detail(format!("source_id: 0x{:x}", s.source_id));
        }
        QueryClass::Origin => {
            let v = payload::parse_origin(bytes).map_err(Error::Decode)?;
            json["decoded"] = json!({ "origin": format!("0x{:016x}", v) });
            lines.detail(format!("origin: 0x{:016x}", v));
        }
        QueryClass::Statistics => {
            let s = payload::parse_statistics(bytes).map_err(Error::Decode)?;
            json["decoded"] = json!({ "fields": s.fields.to_vec() });
            for (i, f) in s.fields.iter().enumerate() {
                lines.detail(format!("field[{i}]: 0x{:016x}", f));
            }
        }
        _ => {
            // No typed decoding for this class yet. Hex dump only.
        }
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

fn parse_class(name: &str) -> Result<QueryClass> {
    match name.to_ascii_lowercase().as_str() {
        "user" => Ok(QueryClass::User),
        "groups" => Ok(QueryClass::Groups),
        "privileges" | "privs" => Ok(QueryClass::Privileges),
        "type" => Ok(QueryClass::Type),
        "integrity" | "integritylevel" | "integrity-level" => Ok(QueryClass::IntegrityLevel),
        "owner" => Ok(QueryClass::Owner),
        "primarygroup" | "primary-group" => Ok(QueryClass::PrimaryGroup),
        "sessionid" | "session-id" | "session" => Ok(QueryClass::SessionId),
        "restrictedsids" | "restricted-sids" => Ok(QueryClass::RestrictedSids),
        "source" => Ok(QueryClass::Source),
        "statistics" | "stats" => Ok(QueryClass::Statistics),
        "origin" => Ok(QueryClass::Origin),
        "elevationtype" | "elevation-type" | "elevation" => Ok(QueryClass::ElevationType),
        "devicegroups" | "device-groups" => Ok(QueryClass::DeviceGroups),
        "appcontainersid" | "appcontainer-sid" | "appcontainer" => Ok(QueryClass::AppContainerSid),
        "capabilities" | "caps" => Ok(QueryClass::Capabilities),
        "mandatorypolicy" | "mandatory-policy" => Ok(QueryClass::MandatoryPolicy),
        "logontype" | "logon-type" => Ok(QueryClass::LogonType),
        "logonsid" | "logon-sid" => Ok(QueryClass::LogonSid),
        "defaultdacl" | "default-dacl" => Ok(QueryClass::DefaultDacl),
        "impersonationlevel" | "impersonation-level" => Ok(QueryClass::ImpersonationLevel),
        "userclaims" | "user-claims" => Ok(QueryClass::UserClaims),
        "deviceclaims" | "device-claims" => Ok(QueryClass::DeviceClaims),
        "projectedsupplementarygids" | "projected-supplementary-gids" | "projected-gids" => {
            Ok(QueryClass::ProjectedSupplementaryGids)
        }
        other => Err(Error::Usage(format!("unknown query class: {other}"))),
    }
}
