// `token adjust ...` — mutate a token's privileges, groups, default
// DACL, or session id.
//
// Each adjustment maps to a single KACS ioctl exposed by libp-token.

use crate::cmd;
use crate::error::{Error, Result};
use crate::privs;
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::target::TargetSpec;
use peios::token::{GroupAdjustment, PrivilegeAdjustment, SessionId, TokenAccess};
use serde_json::json;

const KACS_TOKEN_ADJUST_PRIVS: u32 = TokenAccess::ADJUST_PRIVS.bits();
const KACS_TOKEN_ADJUST_GROUPS: u32 = TokenAccess::ADJUST_GROUPS.bits();
const KACS_TOKEN_ADJUST_DEFAULT: u32 = TokenAccess::ADJUST_DEFAULT.bits();
const KACS_TOKEN_ADJUST_SESSIONID: u32 = TokenAccess::ADJUST_SESSIONID.bits();

const SE_PRIVILEGE_ENABLED: u32 = peios_sys::KACS_PRIVILEGE_ATTR_ENABLED;
const SE_PRIVILEGE_REMOVED: u32 = peios_sys::KACS_PRIVILEGE_ATTR_REMOVED;

// ---------------------------------------------------------------------------
// privs
// ---------------------------------------------------------------------------

pub fn privs(
    matches: &clap::ArgMatches,
    target: TargetSpec,
    mode: OutputMode,
) -> Result<()> {
    let entries: Vec<String> = matches
        .get_many::<String>("entries")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();
    if entries.is_empty() {
        return Err(Error::Usage(
            "adjust privs: pass one or more <name>=<enabled|disabled|removed> entries".into(),
        ));
    }
    let parsed = parse_priv_entries(&entries)?;

    let tok = target.open(KACS_TOKEN_ADJUST_PRIVS)?;
    let previous = tok.adjust_privileges(&parsed)?.bits();

    let mut lines = Lines::new();
    lines.section("adjust privs");
    lines.kv("entries", parsed.len().to_string());
    lines.kv("previous_enabled", format!("0x{previous:016x}"));
    for e in &parsed {
        let name = privs::name_for_bit(e.luid).unwrap_or("?");
        let state = describe_priv_attrs(e.attributes);
        lines.detail(format!("{name} (bit {}) <- {state}", e.luid));
    }
    let arr: Vec<_> = parsed
        .iter()
        .map(|e| {
            json!({
                "bit": e.luid,
                "name": privs::name_for_bit(e.luid),
                "attributes": e.attributes,
                "state": describe_priv_attrs(e.attributes),
            })
        })
        .collect();
    let out = json!({
        "previous_enabled": format!("0x{previous:016x}"),
        "entries": arr,
    });
    cmd::emit(CmdOutput { human: lines, json: out }, mode)
}

fn parse_priv_entries(entries: &[String]) -> Result<Vec<PrivilegeAdjustment>> {
    let mut out = Vec::with_capacity(entries.len());
    for raw in entries {
        let (name_part, state_part) = raw
            .split_once('=')
            .ok_or_else(|| Error::Usage(format!("missing `=state` in `{raw}`")))?;
        let bit = privs::parse_bit(name_part.trim()).map_err(Error::Usage)?;
        let attributes = match state_part.trim().to_ascii_lowercase().as_str() {
            "enabled" | "on" | "true" | "1" => SE_PRIVILEGE_ENABLED,
            "disabled" | "off" | "false" | "0" => 0,
            "removed" | "remove" => SE_PRIVILEGE_REMOVED,
            other => {
                return Err(Error::Usage(format!(
                    "unknown privilege state `{other}` in `{raw}` (expected enabled|disabled|removed)"
                )));
            }
        };
        out.push(PrivilegeAdjustment {
            luid: bit,
            attributes,
        });
    }
    Ok(out)
}

fn describe_priv_attrs(a: u32) -> &'static str {
    if a & SE_PRIVILEGE_REMOVED != 0 {
        "removed"
    } else if a & SE_PRIVILEGE_ENABLED != 0 {
        "enabled"
    } else {
        "disabled"
    }
}

// ---------------------------------------------------------------------------
// groups
// ---------------------------------------------------------------------------

pub fn groups(
    matches: &clap::ArgMatches,
    target: TargetSpec,
    mode: OutputMode,
) -> Result<()> {
    let entries: Vec<String> = matches
        .get_many::<String>("entries")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();
    if entries.is_empty() {
        return Err(Error::Usage(
            "adjust groups: pass one or more <index>=<enabled|disabled> entries".into(),
        ));
    }
    let parsed = parse_group_entries(&entries)?;
    let tok = target.open(KACS_TOKEN_ADJUST_GROUPS)?;
    let previous = tok.adjust_groups(&parsed)?;

    let previous_words: Vec<String> =
        previous.0.iter().map(|w| format!("0x{w:016x}")).collect();
    let mut lines = Lines::new();
    lines.section("adjust groups");
    lines.kv("entries", parsed.len().to_string());
    lines.kv("previous_state", previous_words.join(" "));
    for e in &parsed {
        let state = if e.enable { "enabled" } else { "disabled" };
        lines.detail(format!("idx {} <- {state}", e.index));
    }
    let out = json!({
        "previous_state": previous_words,
        "entries": parsed.iter().map(|e| json!({
            "index": e.index,
            "enable": e.enable,
        })).collect::<Vec<_>>(),
    });
    cmd::emit(CmdOutput { human: lines, json: out }, mode)
}

fn parse_group_entries(entries: &[String]) -> Result<Vec<GroupAdjustment>> {
    let mut out = Vec::with_capacity(entries.len());
    for raw in entries {
        let (idx_part, state_part) = raw
            .split_once('=')
            .ok_or_else(|| Error::Usage(format!("missing `=state` in `{raw}`")))?;
        let index: u32 = idx_part
            .trim()
            .parse()
            .map_err(|_| Error::Usage(format!("bad group index `{idx_part}`")))?;
        let enable = match state_part.trim().to_ascii_lowercase().as_str() {
            "enabled" | "on" | "true" | "1" => true,
            "disabled" | "off" | "false" | "0" => false,
            other => {
                return Err(Error::Usage(format!(
                    "unknown group state `{other}` in `{raw}` (expected enabled|disabled)"
                )));
            }
        };
        out.push(GroupAdjustment { index, enable });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// default (DACL + owner/group indices)
// ---------------------------------------------------------------------------

pub fn default(
    matches: &clap::ArgMatches,
    target: TargetSpec,
    mode: OutputMode,
) -> Result<()> {
    let sddl_str = matches.get_one::<String>("dacl").cloned();
    let owner_idx = matches.get_one::<u16>("owner-idx").copied();
    let group_idx = matches.get_one::<u16>("group-idx").copied();

    if sddl_str.is_none() && owner_idx.is_none() && group_idx.is_none() {
        return Err(Error::Usage(
            "adjust default: pass at least one of --dacl, --owner-idx, --group-idx".into(),
        ));
    }

    let dacl: Option<peios::security::Acl> = match &sddl_str {
        Some(s) => Some(parse_dacl(s)?),
        None => None,
    };

    let tok = target.open(KACS_TOKEN_ADJUST_DEFAULT)?;
    // The new typed `adjust_default` takes `Option<u16>` indices directly,
    // using `None` for "leave unchanged" (libpeios' 0xFFFF sentinel is applied
    // internally), so we pass the parsed options through unmapped.
    tok.adjust_default(dacl.as_ref(), owner_idx, group_idx)?;

    let mut lines = Lines::new();
    lines.section("adjust default");
    if let Some(s) = &sddl_str {
        lines.kv("dacl_sddl", s.clone());
    }
    if let Some(i) = owner_idx {
        lines.kv("owner_idx", i.to_string());
    }
    if let Some(i) = group_idx {
        lines.kv("group_idx", i.to_string());
    }
    let out = json!({
        "dacl_sddl": sddl_str,
        "owner_idx": owner_idx,
        "group_idx": group_idx,
    });
    cmd::emit(CmdOutput { human: lines, json: out }, mode)
}

fn parse_dacl(sddl: &str) -> Result<peios::security::Acl> {
    use peios::security::{Ace, AclBuilder};

    // The SDDL parser emits a full security descriptor; KACS adjust_default
    // takes just an ACL, so we parse the SD, pull its DACL view, and rebuild a
    // standalone `Acl` from the DACL's ACEs (the new `Acl` has no public
    // raw-bytes constructor, so we re-add each ACE through `AclBuilder`).
    let sd = peios::security::sddl::parse(sddl)
        .map_err(|e| Error::Usage(format!("bad SDDL: {e}")))?;
    let view = sd
        .view()
        .map_err(|e| Error::Usage(format!("parsed SD did not round-trip: {e}")))?;
    let dacl = view
        .dacl()
        .ok_or_else(|| Error::Usage("SDDL had no DACL".into()))?;

    let mut builder = AclBuilder::new();
    for ace in dacl.iter() {
        let sid = ace
            .sid()
            .ok_or_else(|| Error::Usage("DACL ACE had no SID".into()))?;
        builder.add(&Ace {
            ace_type: ace.ace_type(),
            flags: ace.flags(),
            mask: ace.mask(),
            sid,
            object_type: ace.object_type(),
            inherited_object_type: ace.inherited_object_type(),
            app_data: ace.app_data(),
        });
    }
    builder
        .build()
        .map_err(|e| Error::Usage(format!("could not encode DACL: {e}")))
}

// ---------------------------------------------------------------------------
// session
// ---------------------------------------------------------------------------

pub fn session(
    matches: &clap::ArgMatches,
    target: TargetSpec,
    mode: OutputMode,
) -> Result<()> {
    let new_id = *matches
        .get_one::<u32>("session-id")
        .ok_or_else(|| Error::Usage("adjust session: missing <id>".into()))?;
    let tok = target.open(KACS_TOKEN_ADJUST_SESSIONID)?;
    tok.set_session_id(SessionId(new_id as u64))?;

    let mut lines = Lines::new();
    lines.section("adjust session");
    lines.kv("new_session_id", new_id.to_string());
    let out = json!({ "new_session_id": new_id });
    cmd::emit(CmdOutput { human: lines, json: out }, mode)
}
