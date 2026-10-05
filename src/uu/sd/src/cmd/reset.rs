// `sd reset <path>` — drop local explicit DACL ACEs and SE_DACL_PROTECTED,
// then re-inherit from parent as KACS creates (reinherit_with). Supports
// --recursive (each target's parent path is computed independently).

use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::target::PathTarget;
use crate::walk;
use clap::ArgMatches;
use peios::file::{File, SecInfo, get_sd, set_sd};
use peios::security::{AclBuilder, Control, SdBuilder, SdView, reinherit_with};
use serde_json::json;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let primary = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);
    let recursive = walk::parse_recursive(matches);

    let targets = walk::walk_paths(&primary, recursive)?;
    let mut errors: Vec<(String, Error)> = Vec::new();
    for target in &targets {
        if let Err(e) = apply_one(target) {
            errors.push((target.path.clone(), e));
        }
    }

    match mode {
        OutputMode::Human => println!(
            "{}: re-inherited on {} target(s)",
            primary.path,
            targets.len()
        ),
        OutputMode::Json => println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "path": primary.path,
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

fn apply_one(target: &PathTarget) -> Result<()> {
    let child_is_container = walk::is_container(&target.path);
    let parent_path = walk::parent_path(&target.path);
    let parent_target = PathTarget {
        path: parent_path,
        no_follow_symlinks: target.no_follow_symlinks,
    };
    let parent_sd = get_sd(
        parent_target.dirfd(),
        parent_target.as_path(),
        SecInfo::DACL,
        parent_target.at_flags(),
    )
    .map_err(Error::from)?;
    let child_sd = get_sd(target.dirfd(), target.as_path(), SecInfo::DACL | SecInfo::OWNER | SecInfo::GROUP, target.at_flags())
        .map_err(Error::from)?;
    if child_sd.as_bytes().is_empty() {
        return Err(Error::Invalid(format!(
            "{}: no SD recorded; nothing to reset",
            target.path
        )));
    }
    // Its owner and group, which CREATOR OWNER and CREATOR GROUP resolve
    // to, an empty DACL, and no protection: what is inherited is all it has.
    let view = SdView::parse(child_sd.as_bytes()).map_err(Error::from)?;
    let mut bare = SdBuilder::new();
    if let Some(owner) = view.owner() {
        bare.owner(owner);
    }
    if let Some(group) = view.group() {
        bare.group(group);
    }
    bare.dacl(&AclBuilder::new().build().map_err(Error::from)?);
    bare.control(Control::empty(), Control::DACL_PROTECTED);
    let bare = bare.build().map_err(Error::from)?;
    let child_bytes = bare.as_bytes();
    let empty;
    let parent_for_inherit: &[u8] = if parent_sd.as_bytes().is_empty() {
        empty = crate::cmd::empty_self_relative_sd();
        &empty
    } else {
        parent_sd.as_bytes()
    };
    let mapping = File::generic_mapping();
    let new_sd = reinherit_with(parent_for_inherit, child_bytes, child_is_container, Some(&mapping), SecInfo::DACL)
        .map_err(Error::from)?;
    set_sd(
        target.dirfd(),
        target.as_path(),
        SecInfo::DACL,
        &new_sd,
        target.at_flags(),
    )
    .map_err(Error::from)?;
    Ok(())
}
