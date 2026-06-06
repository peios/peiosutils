// `token adjust ...` — mutate a token's privileges, groups, default
// DACL, or session id.
//
// Each adjustment maps to a single KACS ioctl exposed by libp-token.

use crate::cmd;
use crate::error::{Error, Result};
use crate::privs;
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::target::TargetSpec;
use libp_token::uapi::{
    KACS_TOKEN_ADJUST_DEFAULT, KACS_TOKEN_ADJUST_GROUPS, KACS_TOKEN_ADJUST_PRIVS,
    KACS_TOKEN_ADJUST_SESSIONID, KacsGroupEntry, KacsPrivEntry, SE_PRIVILEGE_ENABLED,
    SE_PRIVILEGE_REMOVED,
};
use serde_json::json;

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
    let previous = tok.adjust_privs(&parsed)?;

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

fn parse_priv_entries(entries: &[String]) -> Result<Vec<KacsPrivEntry>> {
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
        out.push(KacsPrivEntry {
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
        previous.iter().map(|w| format!("0x{w:016x}")).collect();
    let mut lines = Lines::new();
    lines.section("adjust groups");
    lines.kv("entries", parsed.len().to_string());
    lines.kv("previous_state", previous_words.join(" "));
    for e in &parsed {
        let state = if e.enable != 0 { "enabled" } else { "disabled" };
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

fn parse_group_entries(entries: &[String]) -> Result<Vec<KacsGroupEntry>> {
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
            "enabled" | "on" | "true" | "1" => 1u32,
            "disabled" | "off" | "false" | "0" => 0u32,
            other => {
                return Err(Error::Usage(format!(
                    "unknown group state `{other}` in `{raw}` (expected enabled|disabled)"
                )));
            }
        };
        out.push(KacsGroupEntry { index, enable });
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

    let dacl_bytes: Option<Vec<u8>> = match &sddl_str {
        Some(s) => Some(parse_dacl_to_bytes(s)?),
        None => None,
    };

    let tok = target.open(KACS_TOKEN_ADJUST_DEFAULT)?;
    let owner = owner_idx.unwrap_or(u16::MAX);
    let group = group_idx.unwrap_or(u16::MAX);
    tok.adjust_default(dacl_bytes.as_deref(), owner, group)?;

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

fn parse_dacl_to_bytes(sddl: &str) -> Result<Vec<u8>> {
    // The SDDL parser produces an `SdBuilder` that emits a full
    // SECURITY_DESCRIPTOR. KACS adjust_default takes just an ACL,
    // not a full SD. We extract the DACL bytes from the built SD.
    let sd_bytes = libp_sd::sddl::parse(sddl)
        .map_err(|e| Error::Usage(format!("bad SDDL: {e:?}")))?
        .build()
        .map_err(|e| Error::Usage(format!("could not encode SD: {e:?}")))?;
    let sd = libp_sd::SecurityDescriptor::parse(&sd_bytes)
        .map_err(|e| Error::Usage(format!("parsed SD did not round-trip: {e:?}")))?;
    match sd.dacl() {
        Some(Ok(acl)) => Ok(acl.bytes.to_vec()),
        Some(Err(e)) => Err(Error::Usage(format!("DACL did not parse: {e:?}"))),
        None => Err(Error::Usage("SDDL had no DACL".into())),
    }
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
    tok.adjust_session_id(new_id)?;

    let mut lines = Lines::new();
    lines.section("adjust session");
    lines.kv("new_session_id", new_id.to_string());
    let out = json!({ "new_session_id": new_id });
    cmd::emit(CmdOutput { human: lines, json: out }, mode)
}
