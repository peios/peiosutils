// `sd integrity <path> <level> [--policy NW,NR,NX]` — set the mandatory
// integrity label. Supports --recursive.

use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::target::PathTarget;
use crate::walk;
use clap::ArgMatches;
use libp_sd::{AceBuilder, AclBuilder, SdBuilder, SecurityInfo, WellKnownSid, set_sd};
use serde_json::json;

const POLICY_NW: u32 = 0x0000_0001;
const POLICY_NR: u32 = 0x0000_0002;
const POLICY_NX: u32 = 0x0000_0004;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let primary = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);
    let recursive = walk::parse_recursive(matches);
    let level_str = matches
        .get_one::<String>("level")
        .ok_or_else(|| Error::Usage("missing LEVEL".into()))?;
    let level = parse_level(level_str)?;
    let policy = match matches.get_one::<String>("policy") {
        Some(s) => parse_policy(s)?,
        None => POLICY_NW,
    };

    let targets = walk::walk_paths(&primary, recursive)?;
    let mut errors: Vec<(String, Error)> = Vec::new();
    for target in &targets {
        if let Err(e) = apply_one(target, level, policy) {
            errors.push((target.path.clone(), e));
        }
    }

    match mode {
        OutputMode::Human => println!(
            "{}: integrity set to {} (policy 0x{:x}) on {} target(s)",
            primary.path,
            level.label(),
            policy,
            targets.len()
        ),
        OutputMode::Json => println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "path": primary.path,
                "level": level.label(),
                "policy": format!("0x{:08x}", policy),
                "targets": targets.len(),
                "errors": errors.iter().map(|(p, e)| json!({ "path": p, "error": e.to_string() })).collect::<Vec<_>>(),
            }))
            .unwrap()
        ),
    }
    if !errors.is_empty() {
        return Err(Error::Usage(format!("{} target(s) failed", errors.len())));
    }
    Ok(())
}

fn apply_one(target: &PathTarget, level: WellKnownSid, policy: u32) -> Result<()> {
    let sacl = AclBuilder::new().ace(AceBuilder::mandatory_label(level, policy).flags(0));
    let bytes = SdBuilder::new()
        .sacl(sacl)
        .build()
        .map_err(|e| Error::Invalid(format!("building SD: {e}")))?;
    set_sd(&target.as_sd_target(), SecurityInfo::label(), &bytes).map_err(Error::from)?;
    Ok(())
}

fn parse_level(s: &str) -> Result<WellKnownSid> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "untrusted" => WellKnownSid::UntrustedIl,
        "low" => WellKnownSid::LowIl,
        "medium" | "med" => WellKnownSid::MediumIl,
        "medium-plus" | "mediumplus" | "med-plus" => WellKnownSid::MediumPlusIl,
        "high" => WellKnownSid::HighIl,
        "system" | "sys" => WellKnownSid::SystemIl,
        "protected" | "protected-process" => WellKnownSid::ProtectedProcessIl,
        other => {
            return Err(Error::Usage(format!(
                "unknown integrity level `{other}` (use untrusted|low|medium|medium-plus|high|system|protected)"
            )));
        }
    })
}

fn parse_policy(s: &str) -> Result<u32> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        return Ok(0);
    }
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16)
            .map_err(|_| Error::Usage(format!("bad hex policy `{t}`")));
    }
    let mut mask = 0u32;
    for piece in t.split(',') {
        mask |= match piece.trim().to_ascii_uppercase().as_str() {
            "NW" => POLICY_NW,
            "NR" => POLICY_NR,
            "NX" => POLICY_NX,
            other => {
                return Err(Error::Usage(format!(
                    "unknown policy bit `{other}` (use NW,NR,NX)"
                )));
            }
        };
    }
    Ok(mask)
}
