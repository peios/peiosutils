// `token show` — the headline inspection command.
//
// Variants:
//   - Short: one-line summary (principal, label-if-known, session id)
//   - Default: principal block (user/owner/group/integrity/type/level/session/elevation)
//   - All: everything `show` knows plus groups, privs, claims, capabilities

use crate::cmd;
use crate::error::Result;
use crate::payload::{self, group_attrs_labels};
use crate::privs::{self, PrivSnapshot};
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::sid_render::{self, SidStyle};
use crate::target::TargetSpec;
use peios::security::{Sid, SidRef};
use peios::token::{Token, TokenAccess, TokenClass};
use serde_json::json;

const KACS_TOKEN_QUERY: u32 = TokenAccess::QUERY.bits();

/// Decode a SID-valued query class, matching the old typed `*_sid` accessors.
fn query_sid(tok: &Token, class: TokenClass) -> Result<Sid> {
    let buf = tok.query(class)?;
    SidRef::from_bytes(&buf)
        .map(SidRef::to_sid)
        .ok_or_else(|| crate::error::Error::Decode("invalid SID payload".into()))
}

/// Decode a u32-valued query class.
fn query_u32(tok: &Token, class: TokenClass) -> Result<u32> {
    let buf = tok.query(class)?;
    payload::parse_u32(&buf).map_err(crate::error::Error::Decode)
}

#[derive(Debug, Clone, Copy)]
pub enum ShowKind {
    Short,
    Default,
    All,
}

impl ShowKind {
    pub fn pick(short: bool, all: bool) -> Self {
        match (short, all) {
            (_, true) => ShowKind::All,
            (true, _) => ShowKind::Short,
            _ => ShowKind::Default,
        }
    }
}

pub fn run(target: TargetSpec, style: SidStyle, mode: OutputMode, kind: ShowKind) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let out = build_output(&tok, style, kind, &target)?;
    cmd::emit(out, mode)
}

fn build_output(
    tok: &Token,
    style: SidStyle,
    kind: ShowKind,
    target: &TargetSpec,
) -> Result<CmdOutput> {
    let user = tok.user()?;
    let mut json = json!({
        "target": target.label(),
        "user": sid_render::render_json(&user),
    });
    let mut lines = Lines::new();

    match kind {
        ShowKind::Short => {
            lines.kv("user", sid_render::render(&user, style));
            if let Ok(sid) = tok.session_id() {
                lines.kv("session", sid.0.to_string());
                json["session_id"] = sid.0.into();
            }
            Ok(CmdOutput { human: lines, json })
        }
        ShowKind::Default => {
            fill_principal_block(tok, &mut lines, &mut json, style)?;
            Ok(CmdOutput { human: lines, json })
        }
        ShowKind::All => {
            fill_principal_block(tok, &mut lines, &mut json, style)?;
            fill_groups(tok, &mut lines, &mut json, style)?;
            fill_privs(tok, &mut lines, &mut json)?;
            fill_caps(tok, &mut lines, &mut json, style)?;
            Ok(CmdOutput { human: lines, json })
        }
    }
}

fn fill_principal_block(
    tok: &Token,
    lines: &mut Lines,
    json: &mut serde_json::Value,
    style: SidStyle,
) -> Result<()> {
    lines.section("principal");
    let user = tok.user()?;
    lines.sid("user", &user, style);
    json["user"] = sid_render::render_json(&user);

    if let Ok(owner) = query_sid(tok, TokenClass(peios_sys::KACS_TOKEN_CLASS_OWNER)) {
        lines.sid("owner", &owner, style);
        json["owner"] = sid_render::render_json(&owner);
    }
    if let Ok(group) = query_sid(tok, TokenClass(peios_sys::KACS_TOKEN_CLASS_PRIMARY_GROUP)) {
        lines.sid("primary_group", &group, style);
        json["primary_group"] = sid_render::render_json(&group);
    }
    if let Ok(integrity) = query_sid(tok, TokenClass(peios_sys::KACS_TOKEN_CLASS_INTEGRITY_LEVEL)) {
        lines.sid("integrity", &integrity, style);
        json["integrity"] = sid_render::render_json(&integrity);
    }
    if let Ok(t) = tok.token_type() {
        let s = format!("{t:?}");
        lines.kv("type", s.clone());
        json["type"] = s.into();
    }
    // The new typed Token has no impersonation-level / elevation-type
    // accessor, so these are read raw as the underlying u32 and surfaced as
    // the raw class value (the old code printed the libp enum's Debug form).
    if let Ok(level) =
        query_u32(tok, TokenClass(peios_sys::KACS_TOKEN_CLASS_IMPERSONATION_LEVEL))
    {
        let s = level.to_string();
        lines.kv("impersonation_level", s.clone());
        json["impersonation_level"] = level.into();
    }
    if let Ok(elev) = query_u32(tok, TokenClass(peios_sys::KACS_TOKEN_CLASS_ELEVATION_TYPE)) {
        let s = elev.to_string();
        lines.kv("elevation_type", s.clone());
        json["elevation_type"] = elev.into();
    }
    if let Ok(sid) = tok.session_id() {
        lines.kv("session_id", sid.0.to_string());
        json["session_id"] = sid.0.into();
    }
    Ok(())
}

fn fill_groups(
    tok: &Token,
    lines: &mut Lines,
    json: &mut serde_json::Value,
    style: SidStyle,
) -> Result<()> {
    let buf = tok.query(TokenClass::GROUPS)?;
    let entries = payload::parse_sid_attrs_list(&buf)
        .map_err(crate::error::Error::Decode)?;

    lines.section(format!("groups ({})", entries.len()));
    let mut arr = Vec::new();
    for e in &entries {
        let attrs = group_attrs_labels(e.attributes);
        lines.kv(
            sid_render::render(&e.sid, style),
            attrs.join(", "),
        );
        let mut obj = sid_render::render_json(&e.sid);
        if let Some(o) = obj.as_object_mut() {
            o.insert("attributes_raw".into(), e.attributes.into());
            o.insert("attributes".into(), serde_json::Value::Array(
                attrs.iter().map(|s| (*s).into()).collect()
            ));
        }
        arr.push(obj);
    }
    json["groups"] = serde_json::Value::Array(arr);
    Ok(())
}

fn fill_privs(tok: &Token, lines: &mut Lines, json: &mut serde_json::Value) -> Result<()> {
    let buf = tok.query(TokenClass::PRIVILEGES)?;
    let snap = privs::decode_privs_payload(&buf).map_err(crate::error::Error::Decode)?;
    render_privs_into(&snap, lines, json);
    Ok(())
}

fn render_privs_into(snap: &PrivSnapshot, lines: &mut Lines, json: &mut serde_json::Value) {
    let entries: Vec<privs::PrivEntry> = snap.entries().collect();
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
        lines.kv(e.label(), tags.join(", "));
        arr.push(json!({
            // null rather than a placeholder: a consumer parsing this should be
            // able to tell "this build has no name for bit 40" from a privilege
            // actually called that.
            "name": e.name,
            "bit": e.bit,
            "enabled": e.enabled,
            "enabled_by_default": e.enabled_by_default,
            "used": e.used,
        }));
    }
    json["privileges"] = serde_json::Value::Array(arr);
    json["privileges_raw"] = json!({
        "present": format!("0x{:016x}", snap.present),
        "enabled": format!("0x{:016x}", snap.enabled),
        "enabled_by_default": format!("0x{:016x}", snap.enabled_by_default),
        "used": format!("0x{:016x}", snap.used),
    });
}

fn fill_caps(
    tok: &Token,
    lines: &mut Lines,
    json: &mut serde_json::Value,
    style: SidStyle,
) -> Result<()> {
    let buf = match tok.query(TokenClass::CAPABILITIES) {
        Ok(b) => b,
        // EINVAL: no caps on this token.
        Err(e) if e.raw_os_error() == Some(22) => {
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
    let entries = payload::parse_sid_attrs_list(&buf)
        .map_err(crate::error::Error::Decode)?;
    if entries.is_empty() {
        return Ok(());
    }

    lines.section(format!("capabilities ({})", entries.len()));
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
    json["capabilities"] = serde_json::Value::Array(arr);
    Ok(())
}
