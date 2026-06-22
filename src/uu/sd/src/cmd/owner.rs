// `sd owner` and `sd group` — set the SD's owner/group SID. Supports
// --recursive.

use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use crate::principal;
use crate::target::PathTarget;
use crate::walk;
use clap::ArgMatches;
use peios::file::{SecInfo, set_sd};
use peios::security::{SdBuilder, Sid};
use serde_json::json;

#[derive(Debug, Clone, Copy)]
pub enum Field {
    Owner,
    Group,
}

impl Field {
    fn info(self) -> SecInfo {
        match self {
            Field::Owner => SecInfo::OWNER,
            Field::Group => SecInfo::GROUP,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Field::Owner => "owner",
            Field::Group => "group",
        }
    }
}

pub fn run(matches: &ArgMatches, field: Field) -> Result<()> {
    let primary = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);
    let recursive = walk::parse_recursive(matches);

    let principal_str = matches
        .get_one::<String>("principal")
        .ok_or_else(|| Error::Usage("missing PRINCIPAL".into()))?;
    let sid = principal::parse(principal_str)?;

    let targets = walk::walk_paths(&primary, recursive)?;
    let mut errors: Vec<(String, Error)> = Vec::new();
    for target in &targets {
        if let Err(e) = apply_one(target, field, sid.clone()) {
            errors.push((target.path.clone(), e));
        }
    }

    match mode {
        OutputMode::Human => println!(
            "{}: {} set to {} on {} target(s)",
            primary.path,
            field.name(),
            sid,
            targets.len()
        ),
        OutputMode::Json => println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "path": primary.path,
                "field": field.name(),
                "sid": sid.to_string(),
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

fn apply_one(target: &PathTarget, field: Field, sid: Sid) -> Result<()> {
    let mut sd = SdBuilder::new();
    match field {
        Field::Owner => {
            sd.owner(&sid);
        }
        Field::Group => {
            sd.group(&sid);
        }
    };
    let bytes = sd
        .build()
        .map_err(|e| Error::Invalid(format!("building SD: {e}")))?;
    set_sd(
        target.dirfd(),
        target.as_path(),
        field.info(),
        &bytes,
        target.at_flags(),
    )
    .map_err(Error::from)?;
    Ok(())
}
