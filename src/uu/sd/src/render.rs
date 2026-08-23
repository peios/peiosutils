// Output rendering — human-readable and JSON.
//
// Migrated to the `peios` crate's zero-copy views: an SD is an `SdView`, ACLs
// are `AclView`, ACEs are `AceView`. SID labelling (the old
// `WellKnownSid::from_sid`/`label`) is re-expressed as a reverse lookup from
// canonical SID string to a human label. ACE-type / control-bit naming (the old
// `ace_type_name` / `control_bit_names` helpers) is provided locally.

use crate::flags;
use crate::perms;
use peios::security::{AceType, AclView, Control, SdView, SidRef};
use serde_json::{Value, json};

// Callback ACE-type wire discriminants (no typed `AceType` variant; modelled as
// `AceType::Other`).
const ACE_TYPE_ACCESS_ALLOWED_CALLBACK: u8 = peios_sys::KACS_ACE_TYPE_ACCESS_ALLOWED_CALLBACK as u8;
const ACE_TYPE_ACCESS_DENIED_CALLBACK: u8 = peios_sys::KACS_ACE_TYPE_ACCESS_DENIED_CALLBACK as u8;
const ACE_TYPE_SYSTEM_AUDIT_CALLBACK: u8 = peios_sys::KACS_ACE_TYPE_SYSTEM_AUDIT_CALLBACK as u8;

// SID rendering comes from `uucore::sid_render` -- the one place that turns a
// SID into text, so `sd`, `ls`, `token` and `revstrm` cannot disagree about
// what a principal is called. This file used to carry its own SidStyle and its
// own 17-entry table, which said `LocalSystem` where `token`'s said
// `Local System`.
pub use uucore::sid_render::{SidStyle, render as sid_human};

/// JSON representation of a SID — always emits both `sid` and `label`
/// fields (label = null if no well-known match).
pub fn sid_json(sid: &SidRef) -> Value {
    let raw = sid.to_string();
    let label = uucore::sid_render::name_for(sid, &raw);
    json!({ "sid": raw, "label": label })
}

/// A short, stable name for an ACE type (mirrors the old `ace_type_name`).
fn ace_type_name(t: AceType) -> String {
    match t {
        AceType::AccessAllowed => "ACCESS_ALLOWED".into(),
        AceType::AccessDenied => "ACCESS_DENIED".into(),
        AceType::SystemAudit => "SYSTEM_AUDIT".into(),
        AceType::SystemMandatoryLabel => "SYSTEM_MANDATORY_LABEL".into(),
        AceType::SystemResourceAttribute => "SYSTEM_RESOURCE_ATTRIBUTE".into(),
        AceType::Other(v) if v == ACE_TYPE_ACCESS_ALLOWED_CALLBACK => "ACCESS_ALLOWED_CALLBACK".into(),
        AceType::Other(v) if v == ACE_TYPE_ACCESS_DENIED_CALLBACK => "ACCESS_DENIED_CALLBACK".into(),
        AceType::Other(v) if v == ACE_TYPE_SYSTEM_AUDIT_CALLBACK => "SYSTEM_AUDIT_CALLBACK".into(),
        AceType::Other(v) => format!("OTHER(0x{v:02x})"),
    }
}

/// Translate an ACE type into a short kind ("allow"/"deny"/"audit"/...).
fn ace_kind(t: AceType) -> &'static str {
    match t {
        AceType::AccessAllowed => "allow",
        AceType::AccessDenied => "deny",
        AceType::SystemAudit => "audit",
        AceType::SystemMandatoryLabel => "label",
        AceType::Other(v) if v == ACE_TYPE_ACCESS_ALLOWED_CALLBACK => "allow",
        AceType::Other(v) if v == ACE_TYPE_ACCESS_DENIED_CALLBACK => "deny",
        AceType::Other(v) if v == ACE_TYPE_SYSTEM_AUDIT_CALLBACK => "audit",
        _ => "other",
    }
}

/// Whether an ACE is a plain access ACE (allow/deny/audit, including their
/// callback variants) carrying a (mask, sid) pair we render in detail.
fn is_plain_access(t: AceType) -> bool {
    matches!(
        t,
        AceType::AccessAllowed | AceType::AccessDenied | AceType::SystemAudit
    ) || matches!(t, AceType::Other(v) if v == ACE_TYPE_ACCESS_ALLOWED_CALLBACK
        || v == ACE_TYPE_ACCESS_DENIED_CALLBACK
        || v == ACE_TYPE_SYSTEM_AUDIT_CALLBACK)
}

/// Render an ACL human-side, indented two spaces under a heading the
/// caller emits. Returns the body lines as one string ending with `\n`.
pub fn acl_human(acl: &AclView<'_>, style: SidStyle) -> String {
    let mut out = String::new();
    for (i, ace) in acl.iter().enumerate() {
        let t = ace.ace_type();
        let kind = ace_kind(t);
        if is_plain_access(t) {
            if let Some(sid) = ace.sid() {
                let principal = sid_human(sid, style);
                let perm_str = perms::render(ace.mask());
                let mut tags: Vec<String> = Vec::new();
                let flag_str = flags::render(ace.flags().bits());
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
                continue;
            }
        }
        // Object / label / resource-attribute ACE — render the type name,
        // leave the body opaque for v1.
        let body_len = ace.app_data().map(|d| d.len()).unwrap_or(0);
        out.push_str(&format!(
            "    [{i}] {} ({} bytes body, flags {})\n",
            ace_type_name(t),
            body_len,
            flags::render(ace.flags().bits()),
        ));
    }
    out
}

/// JSON representation of an ACL — array of ACE objects.
pub fn acl_json(acl: &AclView<'_>) -> Value {
    let mut arr: Vec<Value> = Vec::new();
    for (i, ace) in acl.iter().enumerate() {
        let t = ace.ace_type();
        let flags_raw = ace.flags().bits();
        if is_plain_access(t) {
            if let Some(sid) = ace.sid() {
                let mask = ace.mask();
                arr.push(json!({
                    "index": i,
                    "type": ace_kind(t),
                    "ace_type_name": ace_type_name(t),
                    "principal": sid_json(sid),
                    "mask": format!("0x{:08x}", mask),
                    "rights": perms::render(mask),
                    "flags": flags::render(flags_raw).split(',').filter(|s| !s.is_empty()).collect::<Vec<_>>(),
                    "flags_raw": format!("0x{:02x}", flags_raw),
                }));
                continue;
            }
        }
        let body_len = ace.app_data().map(|d| d.len()).unwrap_or(0);
        arr.push(json!({
            "index": i,
            "type": "other",
            "ace_type_name": ace_type_name(t),
            "body_bytes": body_len,
            "flags": flags::render(flags_raw).split(',').filter(|s| !s.is_empty()).collect::<Vec<_>>(),
            "flags_raw": format!("0x{:02x}", flags_raw),
        }));
    }
    Value::Array(arr)
}

/// The set of control bit names set in `c` (mirrors the old `control_bit_names`).
fn control_bit_names(c: Control) -> Vec<&'static str> {
    const NAMES: &[(Control, &str)] = &[
        (Control::DACL_PRESENT, "DACL_PRESENT"),
        (Control::SACL_PRESENT, "SACL_PRESENT"),
        (Control::DACL_PROTECTED, "DACL_PROTECTED"),
        (Control::SACL_PROTECTED, "SACL_PROTECTED"),
        (Control::DACL_AUTO_INHERITED, "DACL_AUTO_INHERITED"),
        (Control::SACL_AUTO_INHERITED, "SACL_AUTO_INHERITED"),
        (Control::SELF_RELATIVE, "SELF_RELATIVE"),
    ];
    NAMES
        .iter()
        .filter(|(bit, _)| c.contains(*bit))
        .map(|(_, n)| *n)
        .collect()
}

/// Render the SD's control word human-side.
pub fn control_human(c: Control) -> String {
    let names = control_bit_names(c);
    if names.is_empty() {
        format!("(none, 0x{:04x})", c.bits())
    } else {
        names.join(", ")
    }
}

/// Render an absent-SD placeholder — used when `get_sd` returns zero
/// bytes. Distinct from a present SD with empty components.
pub fn no_sd_human(path: &str) -> String {
    format!("{path}\n  (no SD set — implicit kernel default applies)\n")
}

pub fn no_sd_json(path: &str) -> Value {
    json!({ "path": path, "sd": null })
}

/// Render a parsed SD as human-readable.
pub fn sd_human(path: &str, sd: &SdView<'_>, style: SidStyle) -> String {
    let mut out = String::new();
    out.push_str(&format!("{path}\n"));
    if let Some(owner) = sd.owner() {
        out.push_str(&format!("  Owner:      {}\n", sid_human(owner, style)));
    } else {
        out.push_str("  Owner:      (absent)\n");
    }
    if let Some(group) = sd.group() {
        out.push_str(&format!("  Group:      {}\n", sid_human(group, style)));
    } else {
        out.push_str("  Group:      (absent)\n");
    }
    out.push_str(&format!("  Control:    {}\n", control_human(sd.control())));
    match sd.dacl() {
        Some(dacl) => {
            out.push_str(&format!("  DACL: ({} ACEs)\n", dacl.len()));
            out.push_str(&acl_human(&dacl, style));
        }
        None => out.push_str("  DACL:       (absent — implicit full access)\n"),
    }
    match sd.sacl() {
        Some(sacl) => {
            out.push_str(&format!("  SACL: ({} ACEs)\n", sacl.len()));
            out.push_str(&acl_human(&sacl, style));
        }
        None => out.push_str("  SACL:       (absent)\n"),
    }
    out
}

/// Render a parsed SD as JSON.
pub fn sd_json(path: &str, sd: &SdView<'_>) -> Value {
    let owner = sd.owner().map(sid_json);
    let group = sd.group().map(sid_json);
    let dacl = sd.dacl().map(|acl| acl_json(&acl));
    let sacl = sd.sacl().map(|acl| acl_json(&acl));
    let control = sd.control();
    json!({
        "path": path,
        "owner": owner,
        "group": group,
        "control": control_bit_names(control),
        "control_raw": format!("0x{:04x}", control.bits()),
        "dacl": dacl,
        "sacl": sacl,
    })
}
