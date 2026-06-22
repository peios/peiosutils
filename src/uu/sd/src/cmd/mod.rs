// Top-level dispatch.

use crate::error::{Error, Result};
use crate::render::SidStyle;
use crate::target::PathTarget;
use clap::ArgMatches;

pub mod allow;
pub mod audit;
pub mod check;
pub mod dacl;
pub mod inherit;
pub mod integrity;
pub mod owner;
pub mod propagate;
pub mod remove;
pub mod reset;
pub mod set;
pub mod show;
pub mod unaudit;

#[derive(Debug, Clone, Copy)]
pub enum OutputMode {
    Human,
    Json,
}

pub fn dispatch(matches: &ArgMatches) -> Result<()> {
    let Some((name, sub)) = matches.subcommand() else {
        return Err(Error::Usage(
            "subcommand required (try `sd --help`)".into(),
        ));
    };
    match name {
        "show" => show::run(sub),
        "check" => check::run(sub),
        "allow" => allow::run(sub, allow::Kind::Allow),
        "deny" => allow::run(sub, allow::Kind::Deny),
        "remove" => remove::run(sub),
        "owner" => owner::run(sub, owner::Field::Owner),
        "group" => owner::run(sub, owner::Field::Group),
        "integrity" => integrity::run(sub),
        "inherit" => inherit::run(sub),
        "reset" => reset::run(sub),
        "set" => set::run(sub),
        "audit" => audit::run(sub),
        "unaudit" => unaudit::run(sub),
        "propagate" => propagate::run(sub),
        other => Err(Error::Usage(format!("unknown subcommand: {other}"))),
    }
}

/// Common: extract a path target from the matches.
pub fn parse_path_target(matches: &ArgMatches) -> Result<PathTarget> {
    let path = matches
        .get_one::<String>("path")
        .ok_or_else(|| Error::Usage("missing PATH".into()))?
        .clone();
    let no_follow_symlinks = matches.get_flag("no-follow-symlinks");
    Ok(PathTarget {
        path,
        no_follow_symlinks,
    })
}

/// Common: parse the --json flag.
pub fn parse_output_mode(matches: &ArgMatches) -> OutputMode {
    if matches.get_flag("json") {
        OutputMode::Json
    } else {
        OutputMode::Human
    }
}

/// A minimal empty self-relative SD (header only, no components). Used by
/// `reset` / `propagate` as the "parent" when the real parent has no SD,
/// driving `reinherit` to strip the child's stale inherited ACEs.
pub fn empty_self_relative_sd() -> Vec<u8> {
    let header = peios_sys::KACS_SD_HEADER_BYTES as usize;
    let self_relative = peios_sys::KACS_SD_SELF_RELATIVE as u16;
    let mut out = vec![0u8; header];
    out[0] = 1; // revision
    out[2..4].copy_from_slice(&self_relative.to_le_bytes());
    out
}

/// Common: parse --raw / --label flags (for commands that show SIDs).
pub fn parse_sid_style(matches: &ArgMatches) -> Result<SidStyle> {
    let raw = matches.try_get_one::<bool>("raw").ok().flatten().copied().unwrap_or(false);
    let label = matches
        .try_get_one::<bool>("label")
        .ok()
        .flatten()
        .copied()
        .unwrap_or(false);
    match (raw, label) {
        (true, true) => Err(Error::Usage(
            "--raw and --label are mutually exclusive".into(),
        )),
        (true, false) => Ok(SidStyle::Raw),
        (false, true) => Ok(SidStyle::Label),
        (false, false) => Ok(SidStyle::Both),
    }
}
