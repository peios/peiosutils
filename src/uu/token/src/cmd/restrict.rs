// `token restrict` — produce a restricted variant of a token.
//
// Restrict-payload wire format (kernel-side, parse_restrict_payload):
//   - `num_deny_indices * u32` (LE) — indices into the source token's
//     group list to mark deny-only
//   - `num_restrict_sids * Sid` (prefix-parsed)
//
// CLI:
//   --drop-privs <mask|names>   bitmask or comma-separated names
//   --deny <idx,idx,...>        comma-separated group-list indices
//   --restrict <sid,sid,...>    comma-separated SIDs (raw or SDDL alias)
//   --flags <mask>              KACS_RESTRICT_* flag bits

use crate::cmd;
use crate::error::{Error, Result};
use crate::privs;
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::target::TargetSpec;
use libp_token::uapi::Sid;
use libp_token::uapi::{KACS_TOKEN_DUPLICATE, KACS_TOKEN_QUERY};
use serde_json::json;

pub fn run(
    matches: &clap::ArgMatches,
    target: TargetSpec,
    mode: OutputMode,
) -> Result<()> {
    let drop_mask = parse_drop_privs(matches.get_one::<String>("drop-privs"))?;
    let deny_indices = parse_indices(matches.get_one::<String>("deny"))?;
    let restrict_sids = parse_sid_list(matches.get_one::<String>("restrict"))?;
    let flags = *matches.get_one::<u32>("flags").unwrap_or(&0);

    let mut payload = Vec::new();
    for idx in &deny_indices {
        payload.extend_from_slice(&idx.to_le_bytes());
    }
    for sid in &restrict_sids {
        payload.extend_from_slice(&sid.encode());
    }

    // KACS requires the source token be opened with QUERY|DUPLICATE
    // (kernel snapshots privileges/groups out of it). DUPLICATE alone is
    // not enough; we request the union.
    let tok = target.open(KACS_TOKEN_QUERY | KACS_TOKEN_DUPLICATE)?;
    let new_tok = tok.restrict(
        drop_mask,
        deny_indices.len() as u32,
        restrict_sids.len() as u32,
        &payload,
        flags,
    )?;

    let mut lines = Lines::new();
    lines.section("restrict");
    lines.kv("drop_privs", format!("0x{drop_mask:016x}"));
    lines.kv("deny_indices", format!("{:?}", deny_indices));
    lines.kv(
        "restrict_sids",
        restrict_sids
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(", "),
    );
    lines.kv("flags", format!("0x{flags:x}"));
    lines.kv("result_fd", new_tok.as_raw_fd().to_string());

    let raw_fd = new_tok.into_raw_fd();
    let out = json!({
        "drop_privs": format!("0x{drop_mask:016x}"),
        "deny_indices": deny_indices,
        "restrict_sids": restrict_sids.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        "flags": flags,
        "result_fd": raw_fd,
    });
    cmd::emit(CmdOutput { human: lines, json: out }, mode)
}

fn parse_drop_privs(arg: Option<&String>) -> Result<u64> {
    let Some(s) = arg else { return Ok(0) };
    let trimmed = s.trim();

    // Numeric mask (hex or decimal).
    let numeric = if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else if trimmed.chars().all(|c| c.is_ascii_digit()) {
        trimmed.parse::<u64>().ok()
    } else {
        None
    };
    if let Some(mask) = numeric {
        return Ok(mask);
    }

    // Comma-separated names: build mask out of (1 << bit).
    let mut mask: u64 = 0;
    for tok in trimmed.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let bit = privs::parse_bit(tok).map_err(Error::Usage)?;
        mask |= 1u64 << bit;
    }
    Ok(mask)
}

fn parse_indices(arg: Option<&String>) -> Result<Vec<u32>> {
    let Some(s) = arg else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for tok in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let idx: u32 = tok
            .parse()
            .map_err(|_| Error::Usage(format!("bad index `{tok}`")))?;
        out.push(idx);
    }
    Ok(out)
}

fn parse_sid_list(arg: Option<&String>) -> Result<Vec<Sid>> {
    let Some(s) = arg else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for tok in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let sid = libp_sd::sddl::parse_sid(tok)
            .map_err(|e| Error::Usage(format!("bad SID `{tok}`: {e:?}")))?;
        out.push(sid);
    }
    Ok(out)
}
