// `sd inherit <path> on|off` — toggle SE_DACL_PROTECTED. Supports --recursive.

use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::target::PathTarget;
use crate::walk;
use clap::ArgMatches;
use libp_sd::{
    AceBuilder, AclBuilder, SdBuilder, SecurityDescriptor, SecurityInfo, get_sd, set_sd,
};
use libp_sd::consts::{ACE_FLAG_INHERITED, SE_DACL_PROTECTED};
use serde_json::json;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let primary = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);
    let recursive = walk::parse_recursive(matches);
    let mode_str = matches
        .get_one::<String>("mode")
        .ok_or_else(|| Error::Usage("missing on|off".into()))?;
    let strip_inherited = matches.get_flag("strip-inherited");

    let want_protected = match mode_str.to_ascii_lowercase().as_str() {
        "on" => false,
        "off" => true,
        other => {
            return Err(Error::Usage(format!(
                "expected `on` or `off`, got `{other}`"
            )));
        }
    };

    let targets = walk::walk_paths(&primary, recursive)?;
    let mut errors: Vec<(String, Error)> = Vec::new();
    for target in &targets {
        if let Err(e) = apply_one(target, want_protected, strip_inherited) {
            errors.push((target.path.clone(), e));
        }
    }

    match mode {
        OutputMode::Human => println!(
            "{}: inheritance {} on {} target(s)",
            primary.path,
            if want_protected { "OFF" } else { "ON" },
            targets.len()
        ),
        OutputMode::Json => println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "path": primary.path,
                "inheritance": !want_protected,
                "protected": want_protected,
                "stripped_inherited": strip_inherited,
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

fn apply_one(target: &PathTarget, protect: bool, strip_inherited: bool) -> Result<()> {
    let bytes = get_sd(&target.as_sd_target(), SecurityInfo::dacl()).map_err(Error::from)?;
    let mut dacl = AclBuilder::new();
    if !bytes.is_empty() {
        let sd = SecurityDescriptor::parse(&bytes)
            .map_err(|e| Error::Invalid(format!("parsing SD bytes: {e}")))?;
        if let Some(acl_r) = sd.dacl() {
            let acl = acl_r.map_err(|e| Error::Invalid(format!("parsing DACL: {e}")))?;
            for ace_r in acl.aces_iter() {
                let ace = ace_r.map_err(|e| Error::Invalid(format!("parsing ACE: {e}")))?;
                if strip_inherited && ace.flags & ACE_FLAG_INHERITED != 0 {
                    continue;
                }
                dacl = dacl.ace(AceBuilder::from_ace_ref(&ace));
            }
        }
    }
    let mut sd = SdBuilder::new().dacl(dacl);
    if protect {
        sd = sd.control(SE_DACL_PROTECTED);
    }
    let out = sd
        .build()
        .map_err(|e| Error::Invalid(format!("building SD: {e}")))?;
    set_sd(&target.as_sd_target(), SecurityInfo::dacl(), &out).map_err(Error::from)?;
    Ok(())
}
