// Subcommand dispatch. Each subcommand is a small module under
// `cmd::*`. The dispatcher parses target/style flags shared across all
// subcommands and routes to the matching handler.

use crate::error::{Error, Result};
use crate::render::{CmdOutput, OutputMode};
use crate::sid_render::SidStyle;
use crate::target::TargetSpec;
use clap::ArgMatches;

pub mod accessors;
pub mod adjust;
pub mod create;
pub mod dup;
pub mod imp;
pub mod link;
pub mod query;
pub mod restrict;
pub mod show;

/// Top-level dispatch. Returns the rendered output (or an `Error`).
pub fn dispatch(matches: &ArgMatches) -> Result<()> {
    let (name, sub) = match matches.subcommand() {
        Some((n, m)) => (n, m),
        None => {
            // Bare `token` → `token show --short` using the top-level
            // flags (so `token --pid 1` works without typing `show`).
            return run_show(matches, /* implicit_short */ true);
        }
    };

    match name {
        "show" => run_show(sub, false),
        "query" => query::run(sub, parse_target(sub)?, parse_output_mode(sub)),
        "user" => accessors::user(sub, parse_target(sub)?, parse_sid_style(sub), parse_output_mode(sub)),
        "owner" => accessors::owner(sub, parse_target(sub)?, parse_sid_style(sub), parse_output_mode(sub)),
        "group" => accessors::group(sub, parse_target(sub)?, parse_sid_style(sub), parse_output_mode(sub)),
        "privs" => accessors::privs(sub, parse_target(sub)?, parse_output_mode(sub)),
        "groups" => accessors::groups(sub, parse_target(sub)?, parse_sid_style(sub), parse_output_mode(sub)),
        "claims" => accessors::claims(sub, parse_target(sub)?, parse_output_mode(sub)),
        "caps" => accessors::caps(sub, parse_target(sub)?, parse_sid_style(sub), parse_output_mode(sub)),
        "integrity" => accessors::integrity(sub, parse_target(sub)?, parse_sid_style(sub), parse_output_mode(sub)),
        "stats" => accessors::stats(sub, parse_target(sub)?, parse_output_mode(sub)),
        "source" => accessors::source(sub, parse_target(sub)?, parse_output_mode(sub)),
        "origin" => accessors::origin(sub, parse_target(sub)?, parse_output_mode(sub)),
        "logon" => accessors::logon(sub, parse_target(sub)?, parse_sid_style(sub), parse_output_mode(sub)),
        "default-dacl" => accessors::default_dacl(sub, parse_target(sub)?, parse_output_mode(sub)),
        "adjust" => dispatch_adjust(sub),
        "duplicate" | "dup" => dup::run(sub, parse_target(sub)?, parse_output_mode(sub)),
        "restrict" => restrict::run(sub, parse_target(sub)?, parse_output_mode(sub)),
        "link" => link::link(sub, parse_output_mode(sub)),
        "linked" => link::linked(parse_target(sub)?, parse_output_mode(sub)),
        "impersonate" => imp::impersonate(sub, parse_target(sub)?, parse_output_mode(sub)),
        "revert" => imp::revert(parse_output_mode(sub)),
        "create" => create::create(sub, parse_output_mode(sub)),
        "install" => create::install(sub, parse_output_mode(sub)),
        other => Err(Error::Usage(format!("unknown subcommand: {other}"))),
    }
}

fn dispatch_adjust(matches: &ArgMatches) -> Result<()> {
    let (name, sub) = matches
        .subcommand()
        .ok_or_else(|| Error::Usage("adjust: subcommand required (privs|groups|default|session)".into()))?;
    let mode = parse_output_mode(sub);
    let target = parse_target(sub)?;
    match name {
        "privs" => adjust::privs(sub, target, mode),
        "groups" => adjust::groups(sub, target, mode),
        "default" => adjust::default(sub, target, mode),
        "session" => adjust::session(sub, target, mode),
        other => Err(Error::Usage(format!("unknown adjust subcommand: {other}"))),
    }
}

fn run_show(matches: &ArgMatches, implicit_short: bool) -> Result<()> {
    let target = parse_target(matches)?;
    let style = parse_sid_style(matches);
    let mode = parse_output_mode(matches);
    let short = matches.get_flag("short") || implicit_short;
    let all = matches.get_flag("all");
    show::run(target, style, mode, show::ShowKind::pick(short, all))
}

// ---------------------------------------------------------------------------
// Shared flag parsing.
// ---------------------------------------------------------------------------

pub fn parse_target(matches: &ArgMatches) -> Result<TargetSpec> {
    let real = matches.get_flag("real");
    let pid = matches.get_one::<i32>("pid").copied();
    let tid = matches.get_one::<i32>("tid").copied();
    let peer = matches.get_one::<i32>("peer").copied();
    let want_self = matches.get_flag("self");

    let selected = [pid.is_some(), peer.is_some(), want_self].iter().filter(|b| **b).count();
    if selected > 1 {
        return Err(Error::Usage(
            "target flags --self, --pid, --peer are mutually exclusive".into(),
        ));
    }
    if tid.is_some() && pid.is_none() {
        return Err(Error::Usage("--tid requires --pid".into()));
    }

    if let Some(pid) = pid {
        if let Some(tid) = tid {
            return Ok(TargetSpec::Thread { pid, tid });
        }
        return Ok(TargetSpec::Pid(pid));
    }
    if let Some(fd) = peer {
        return Ok(TargetSpec::Peer(fd));
    }
    Ok(TargetSpec::SelfTok { real })
}

pub fn parse_sid_style(matches: &ArgMatches) -> SidStyle {
    match (matches.get_flag("raw"), matches.get_flag("label")) {
        (true, _) => SidStyle::Raw,
        (false, true) => SidStyle::Label,
        _ => SidStyle::Both,
    }
}

pub fn parse_output_mode(matches: &ArgMatches) -> OutputMode {
    if matches.get_flag("json") {
        OutputMode::Json
    } else {
        OutputMode::Human
    }
}

/// Print a rendered command output to stdout.
pub fn emit(out: CmdOutput, mode: OutputMode) -> Result<()> {
    out.print(mode);
    Ok(())
}
