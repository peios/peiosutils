// Output rendering — human-readable and JSON.

use crate::flags;
use crate::perms;
use libp_sd::{Acl, SecurityDescriptor, Sid, WellKnownSid};
use libp_sd::consts::{
    ACE_TYPE_ACCESS_ALLOWED, ACE_TYPE_ACCESS_ALLOWED_CALLBACK, ACE_TYPE_ACCESS_DENIED,
    ACE_TYPE_ACCESS_DENIED_CALLBACK, ACE_TYPE_SYSTEM_AUDIT, ACE_TYPE_SYSTEM_AUDIT_CALLBACK,
    ACE_TYPE_SYSTEM_MANDATORY_LABEL, ace_type_name, control_bit_names,
};
use serde_json::{Value, json};

/// SID rendering style (matches the `--raw`/`--label` flags on `token`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidStyle {
    Both,
    Raw,
    Label,
}

impl Default for SidStyle {
    fn default() -> Self {
        SidStyle::Both
    }
}

/// Render a SID human-side. Returns `label (raw)`, just `raw`, or just `label`.
pub fn sid_human(sid: &Sid, style: SidStyle) -> String {
    let raw = sid.to_string();
    let label = WellKnownSid::from_sid(sid).map(|w| w.label().to_string());
    match (style, label) {
        (SidStyle::Raw, _) | (SidStyle::Both, None) => raw,
        (SidStyle::Label, Some(l)) => l,
        (SidStyle::Label, None) => raw, // fall back
        (SidStyle::Both, Some(l)) => format!("{l} ({raw})"),
    }
}

/// JSON representation of a SID — always emits both `sid` and `label`
/// fields (label = null if no well-known match).
pub fn sid_json(sid: &Sid) -> Value {
    let raw = sid.to_string();
    let label = WellKnownSid::from_sid(sid).map(|w| w.label());
    json!({ "sid": raw, "label": label })
}

/// Translate an ACE type byte into a short kind ("allow"/"deny"/"audit"/...).
fn ace_kind(t: u8) -> &'static str {
    match t {
        ACE_TYPE_ACCESS_ALLOWED | ACE_TYPE_ACCESS_ALLOWED_CALLBACK => "allow",
        ACE_TYPE_ACCESS_DENIED | ACE_TYPE_ACCESS_DENIED_CALLBACK => "deny",
        ACE_TYPE_SYSTEM_AUDIT | ACE_TYPE_SYSTEM_AUDIT_CALLBACK => "audit",
        ACE_TYPE_SYSTEM_MANDATORY_LABEL => "label",
        _ => "other",
    }
}

/// Render an ACL human-side, indented two spaces under a heading the
/// caller emits. Returns the body lines as one string ending with `\n`.
pub fn acl_human(acl: &Acl<'_>, style: SidStyle) -> String {
    let mut out = String::new();
    for (i, ace_r) in acl.aces_iter().enumerate() {
        let Ok(ace) = ace_r else {
            out.push_str(&format!("    [{i}] (malformed ACE)\n"));
            continue;
        };
        let kind = ace_kind(ace.ace_type);
        if let Some((mask, sid)) = ace.as_mask_sid() {
            let sid_owned = sid.to_owned();
            let principal = sid_human(&sid_owned, style);
            let perm_str = perms::render(mask);
            let mut tags: Vec<String> = Vec::new();
            let flag_str = flags::render(ace.flags);
            if !flag_str.is_empty() {
                tags.push(flag_str);
            }
            let tag = if tags.is_empty() {
                String::new()
            } else {
                format!("  [{}]", tags.join(","))
            };
            out.push_str(&format!(
                "    [{i}] {kind:<5} {principal:<40} {perm_str}{tag}\n"
            ));
        } else {
            // Object / callback / resource-attribute ACE — render the type
            // name, leave the body opaque for v1.
            out.push_str(&format!(
                "    [{i}] {} ({} bytes body, flags {})\n",
                ace_type_name(ace.ace_type),
                ace.body.len(),
                flags::render(ace.flags),
            ));
        }
    }
    out
}

/// JSON representation of an ACL — array of ACE objects.
pub fn acl_json(acl: &Acl<'_>) -> Value {
    let mut arr: Vec<Value> = Vec::new();
    for (i, ace_r) in acl.aces_iter().enumerate() {
        let Ok(ace) = ace_r else {
            arr.push(json!({ "index": i, "error": "malformed" }));
            continue;
        };
        if let Some((mask, sid)) = ace.as_mask_sid() {
            let sid_owned = sid.to_owned();
            arr.push(json!({
                "index": i,
                "type": ace_kind(ace.ace_type),
                "ace_type_name": ace_type_name(ace.ace_type),
                "principal": sid_json(&sid_owned),
                "mask": format!("0x{:08x}", mask),
                "rights": perms::render(mask),
                "flags": flags::render(ace.flags).split(',').filter(|s| !s.is_empty()).collect::<Vec<_>>(),
                "flags_raw": format!("0x{:02x}", ace.flags),
            }));
        } else {
            arr.push(json!({
                "index": i,
                "type": "other",
                "ace_type_name": ace_type_name(ace.ace_type),
                "body_bytes": ace.body.len(),
                "flags": flags::render(ace.flags).split(',').filter(|s| !s.is_empty()).collect::<Vec<_>>(),
                "flags_raw": format!("0x{:02x}", ace.flags),
            }));
        }
    }
    Value::Array(arr)
}

/// Render the SD's control word human-side.
pub fn control_human(c: u16) -> String {
    let names = control_bit_names(c);
    if names.is_empty() {
        format!("(none, 0x{:04x})", c)
    } else {
        names.join(", ")
    }
}

/// Render an absent-SD placeholder — used when `kacs_get_sd` returns zero
/// bytes. Distinct from a present SD with empty components.
pub fn no_sd_human(path: &str) -> String {
    format!("{path}\n  (no SD set — implicit kernel default applies)\n")
}

pub fn no_sd_json(path: &str) -> Value {
    json!({ "path": path, "sd": null })
}

/// Render a parsed SD as human-readable.
pub fn sd_human(path: &str, sd: &SecurityDescriptor<'_>, style: SidStyle) -> String {
    let mut out = String::new();
    out.push_str(&format!("{path}\n"));
    if let Some(owner) = sd.owner() {
        out.push_str(&format!("  Owner:      {}\n", sid_human(&owner, style)));
    } else {
        out.push_str("  Owner:      (absent)\n");
    }
    if let Some(group) = sd.group() {
        out.push_str(&format!("  Group:      {}\n", sid_human(&group, style)));
    } else {
        out.push_str("  Group:      (absent)\n");
    }
    out.push_str(&format!("  Control:    {}\n", control_human(sd.control)));
    match sd.dacl() {
        Some(Ok(dacl)) => {
            out.push_str(&format!("  DACL: ({} ACEs)\n", dacl.ace_count));
            out.push_str(&acl_human(&dacl, style));
        }
        Some(Err(e)) => out.push_str(&format!("  DACL: (parse error: {e})\n")),
        None => out.push_str("  DACL:       (absent — implicit full access)\n"),
    }
    match sd.sacl() {
        Some(Ok(sacl)) => {
            out.push_str(&format!("  SACL: ({} ACEs)\n", sacl.ace_count));
            out.push_str(&acl_human(&sacl, style));
        }
        Some(Err(e)) => out.push_str(&format!("  SACL: (parse error: {e})\n")),
        None => out.push_str("  SACL:       (absent)\n"),
    }
    out
}

/// Render a parsed SD as JSON.
pub fn sd_json(path: &str, sd: &SecurityDescriptor<'_>) -> Value {
    let owner = sd.owner().as_ref().map(sid_json);
    let group = sd.group().as_ref().map(sid_json);
    let dacl = match sd.dacl() {
        Some(Ok(acl)) => Some(acl_json(&acl)),
        Some(Err(e)) => Some(json!({ "error": e.to_string() })),
        None => None,
    };
    let sacl = match sd.sacl() {
        Some(Ok(acl)) => Some(acl_json(&acl)),
        Some(Err(e)) => Some(json!({ "error": e.to_string() })),
        None => None,
    };
    json!({
        "path": path,
        "owner": owner,
        "group": group,
        "control": control_bit_names(sd.control),
        "control_raw": format!("0x{:04x}", sd.control),
        "dacl": dacl,
        "sacl": sacl,
    })
}
