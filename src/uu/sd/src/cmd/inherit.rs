// `sd inherit <path> on|off` — toggle DACL_PROTECTED. Supports --recursive.
//
// Hand logic preserved against the new `peios` views: read the DACL, copy its
// ACEs (optionally dropping ACE_FLAG_INHERITED), and rebuild, setting
// `Control::DACL_PROTECTED` when protecting.

use crate::cmd::dacl::AclEdit;
use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::target::PathTarget;
use crate::walk;
use clap::ArgMatches;
use peios::file::{SecInfo, get_sd, set_sd};
use peios::security::{AceFlags, Control, SdBuilder, SdView};
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
    let sd_bytes = get_sd(target.dirfd(), target.as_path(), SecInfo::DACL, target.at_flags())
        .map_err(Error::from)?;
    let bytes = sd_bytes.as_bytes();

    let mut dacl = AclEdit::new();
    if !bytes.is_empty() {
        let view = SdView::parse(bytes)
            .map_err(|e| Error::Invalid(format!("parsing SD bytes: {e}")))?;
        if let Some(acl) = view.dacl() {
            for ace in acl.iter() {
                if strip_inherited && ace.flags().contains(AceFlags::INHERITED) {
                    continue;
                }
                dacl.copy_in(&ace);
            }
        }
    }

    let built = dacl
        .build_public()
        .map_err(|e| Error::Invalid(format!("building DACL: {e}")))?;
    let mut sd = SdBuilder::new();
    sd.dacl(&built);
    if protect {
        sd.control(Control::DACL_PROTECTED, Control::empty());
    } else {
        sd.control(Control::empty(), Control::DACL_PROTECTED);
    }
    let out = sd
        .build()
        .map_err(|e| Error::Invalid(format!("building SD: {e}")))?;
    set_sd(
        target.dirfd(),
        target.as_path(),
        SecInfo::DACL,
        &out,
        target.at_flags(),
    )
    .map_err(Error::from)?;
    Ok(())
}
