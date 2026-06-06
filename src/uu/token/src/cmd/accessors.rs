// Convenience accessors — small one-class readers sugared over
// `token query <class>`. Each prints the relevant value as a focused
// view; `--json` always available.

use crate::cmd;
use crate::error::{Error, Result};
use crate::payload::{self, group_attrs_labels};
use crate::privs;
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::sid_render::{self, SidStyle};
use crate::target::TargetSpec;
use libp_token::{QueryClass, Token};
use libp_token::uapi::SidRef;
use libp_token::uapi::{KACS_TOKEN_QUERY, KACS_TOKEN_QUERY_SOURCE};
use serde_json::json;

// ---------------------------------------------------------------------------
// Single-SID accessors.
// ---------------------------------------------------------------------------

pub fn user(
    _matches: &clap::ArgMatches,
    target: TargetSpec,
    style: SidStyle,
    mode: OutputMode,
) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let sid = tok.user_sid()?;
    emit_sid("user", &sid, style, mode)
}

pub fn owner(
    _matches: &clap::ArgMatches,
    target: TargetSpec,
    style: SidStyle,
    mode: OutputMode,
) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let sid = tok.owner_sid()?;
    emit_sid("owner", &sid, style, mode)
}

pub fn group(
    _matches: &clap::ArgMatches,
    target: TargetSpec,
    style: SidStyle,
    mode: OutputMode,
) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let sid = tok.primary_group_sid()?;
    emit_sid("primary_group", &sid, style, mode)
}

pub fn integrity(
    _matches: &clap::ArgMatches,
    target: TargetSpec,
    style: SidStyle,
    mode: OutputMode,
) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let sid = tok.integrity_level()?;
    emit_sid("integrity", &sid, style, mode)
}

fn emit_sid(
    key: &str,
    sid: &libp_token::uapi::Sid,
    style: SidStyle,
    mode: OutputMode,
) -> Result<()> {
    let mut lines = Lines::new();
    lines.kv(key, sid_render::render(sid, style));
    let json = json!({ key: sid_render::render_json(sid) });
    cmd::emit(CmdOutput { human: lines, json }, mode)
}

// ---------------------------------------------------------------------------
// List accessors.
// ---------------------------------------------------------------------------

pub fn groups(
    _matches: &clap::ArgMatches,
    target: TargetSpec,
    style: SidStyle,
    mode: OutputMode,
) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    sid_attrs_list(&tok, QueryClass::Groups, "groups", style, mode)
}

pub fn caps(
    _matches: &clap::ArgMatches,
    target: TargetSpec,
    style: SidStyle,
    mode: OutputMode,
) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    sid_attrs_list(&tok, QueryClass::Capabilities, "capabilities", style, mode)
}

fn sid_attrs_list(
    tok: &Token,
    class: QueryClass,
    label: &str,
    style: SidStyle,
    mode: OutputMode,
) -> Result<()> {
    let buf = tok.query(class)?;
    let entries = payload::parse_sid_attrs_list(&buf).map_err(Error::Decode)?;

    let mut lines = Lines::new();
    lines.section(format!("{label} ({})", entries.len()));
    let mut arr = Vec::new();
    for e in &entries {
        let attrs = group_attrs_labels(e.attributes);
        lines.kv(sid_render::render(&e.sid, style), attrs.join(", "));
        let mut obj = sid_render::render_json(&e.sid);
        if let Some(o) = obj.as_object_mut() {
            o.insert("attributes_raw".into(), e.attributes.into());
            o.insert(
                "attributes".into(),
                serde_json::Value::Array(attrs.iter().map(|s| (*s).into()).collect()),
            );
        }
        arr.push(obj);
    }
    let json = json!({ label: arr });
    cmd::emit(CmdOutput { human: lines, json }, mode)
}

// ---------------------------------------------------------------------------
// Privileges.
// ---------------------------------------------------------------------------

pub fn privs(_matches: &clap::ArgMatches, target: TargetSpec, mode: OutputMode) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let buf = tok.query(QueryClass::Privileges)?;
    let snap = privs::decode_privs_payload(&buf).map_err(Error::Decode)?;

    let mut lines = Lines::new();
    let entries: Vec<_> = snap.entries().collect();
    lines.section(format!("privileges ({})", entries.len()));
    let mut arr = Vec::new();
    for e in &entries {
        let mut tags = Vec::new();
        tags.push(if e.enabled { "enabled" } else { "disabled" });
        if e.enabled_by_default {
            tags.push("default");
        }
        if e.used {
            tags.push("used");
        }
        lines.kv(e.name.to_string(), tags.join(", "));
        arr.push(json!({
            "name": e.name,
            "bit": e.bit,
            "enabled": e.enabled,
            "enabled_by_default": e.enabled_by_default,
            "used": e.used,
        }));
    }
    let json = json!({
        "privileges": arr,
        "raw": {
            "present": format!("0x{:016x}", snap.present),
            "enabled": format!("0x{:016x}", snap.enabled),
            "enabled_by_default": format!("0x{:016x}", snap.enabled_by_default),
            "used": format!("0x{:016x}", snap.used),
        }
    });
    cmd::emit(CmdOutput { human: lines, json }, mode)
}

// ---------------------------------------------------------------------------
// Scalars / opaque payloads.
// ---------------------------------------------------------------------------

pub fn stats(_matches: &clap::ArgMatches, target: TargetSpec, mode: OutputMode) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let buf = tok.query(QueryClass::Statistics)?;
    let s = payload::parse_statistics(&buf).map_err(Error::Decode)?;
    let mut lines = Lines::new();
    lines.section("statistics");
    for (i, f) in s.fields.iter().enumerate() {
        lines.kv(format!("field[{i}]"), format!("0x{:016x}", f));
    }
    let json = json!({ "fields": s.fields.to_vec() });
    cmd::emit(CmdOutput { human: lines, json }, mode)
}

pub fn source(_matches: &clap::ArgMatches, target: TargetSpec, mode: OutputMode) -> Result<()> {
    // KACS_TOKEN_QUERY_SOURCE is a distinct right.
    let tok = target.open(KACS_TOKEN_QUERY_SOURCE)?;
    let buf = tok.query(QueryClass::Source)?;
    let s = payload::parse_source(&buf).map_err(Error::Decode)?;
    let mut lines = Lines::new();
    lines.section("source");
    let name_str = String::from_utf8_lossy(&s.name).trim_end_matches('\0').to_string();
    lines.kv("name", name_str.clone());
    lines.kv("source_id", format!("0x{:x}", s.source_id));
    let json = json!({
        "name": name_str,
        "name_bytes": s.name.to_vec(),
        "source_id": s.source_id,
    });
    cmd::emit(CmdOutput { human: lines, json }, mode)
}

pub fn origin(_matches: &clap::ArgMatches, target: TargetSpec, mode: OutputMode) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let buf = tok.query(QueryClass::Origin)?;
    let v = payload::parse_origin(&buf).map_err(Error::Decode)?;
    let mut lines = Lines::new();
    lines.kv("origin", format!("0x{:016x}", v));
    let json = json!({ "origin": format!("0x{:016x}", v) });
    cmd::emit(CmdOutput { human: lines, json }, mode)
}

pub fn logon(
    _matches: &clap::ArgMatches,
    target: TargetSpec,
    style: SidStyle,
    mode: OutputMode,
) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let logon_type = payload::parse_u32(&tok.query(QueryClass::LogonType)?)
        .map_err(Error::Decode)?;
    let logon_sid_bytes = tok.query(QueryClass::LogonSid)?;
    let logon_sid = if logon_sid_bytes.is_empty() {
        None
    } else {
        let (sref, _) = SidRef::parse(&logon_sid_bytes)
            .map_err(|e| Error::Decode(format!("logon_sid: {e:?}")))?;
        Some(sref.to_owned())
    };

    let mut lines = Lines::new();
    lines.section("logon");
    lines.kv("logon_type", logon_type.to_string());
    if let Some(sid) = &logon_sid {
        lines.sid("logon_sid", sid, style);
    }
    let mut json = json!({ "logon_type": logon_type });
    if let Some(sid) = &logon_sid {
        json["logon_sid"] = sid_render::render_json(sid);
    }
    cmd::emit(CmdOutput { human: lines, json }, mode)
}

pub fn default_dacl(
    _matches: &clap::ArgMatches,
    target: TargetSpec,
    mode: OutputMode,
) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let buf = tok.query(QueryClass::DefaultDacl)?;
    let mut lines = Lines::new();
    lines.section("default_dacl");
    lines.kv("bytes_len", buf.len().to_string());
    // Hex preview only — a proper DACL parse/render would need
    // libp_sd::sd::Acl scaffolding. Defer pretty-print to a later phase.
    let hex: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    lines.detail(format!("hex: {hex}"));
    let json = json!({
        "bytes_len": buf.len(),
        "bytes_hex": hex,
    });
    cmd::emit(CmdOutput { human: lines, json }, mode)
}

// ---------------------------------------------------------------------------
// Claims — best-effort raw view for now.
// ---------------------------------------------------------------------------

pub fn claims(_matches: &clap::ArgMatches, target: TargetSpec, mode: OutputMode) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let user = tok.query(QueryClass::UserClaims).unwrap_or_default();
    let device = tok.query(QueryClass::DeviceClaims).unwrap_or_default();

    let mut lines = Lines::new();
    lines.section("claims");
    lines.kv("user_claims_bytes", user.len().to_string());
    lines.kv("device_claims_bytes", device.len().to_string());
    if !user.is_empty() {
        lines.detail(format!("user_hex: {}", hex(&user)));
    }
    if !device.is_empty() {
        lines.detail(format!("device_hex: {}", hex(&device)));
    }
    let json = json!({
        "user_claims_bytes_len": user.len(),
        "user_claims_hex": hex(&user),
        "device_claims_bytes_len": device.len(),
        "device_claims_hex": hex(&device),
    });
    cmd::emit(CmdOutput { human: lines, json }, mode)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
