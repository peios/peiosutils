// Subcommand dispatch and the feature lifecycle state machine.
//
// Each transition writes the *pending* state, runs the matching script, then
// writes the *settled* state. So a failed or interrupted script leaves the
// pending value in the registry (e.g. `Installing`) as evidence — never a state
// that claims success that didn't happen:
//
//   NotInstalled --Installing--> Installed --Enabling--> Enabled
//   Enabled --Disabling--> Installed --Uninstalling--> NotInstalled
//
// `add` = install then enable; `remove`/`uninstall` = disable (if on) then
// uninstall. Pending states are treated as "resume/retry": e.g. `install` on a
// feature stuck `Installing` just re-runs install.sh (scripts must be idempotent).

use clap::ArgMatches;

use crate::error::{Error, Result};
use crate::feature::{self, Phase};
use crate::registry::{self, State};

pub fn dispatch(matches: &ArgMatches) -> Result<()> {
    let (name, m) = matches
        .subcommand()
        .ok_or_else(|| Error::Usage("a subcommand is required".into()))?;
    match name {
        "list" => list(),
        "install" => install(&arg_name(m)?),
        "enable" => enable(&arg_name(m)?),
        "disable" => disable(&arg_name(m)?),
        "add" => add(&arg_name(m)?),
        "remove" | "uninstall" => remove(&arg_name(m)?),
        other => Err(Error::Usage(format!("unknown subcommand: {other}"))),
    }
}

fn arg_name(m: &ArgMatches) -> Result<String> {
    let name = m
        .get_one::<String>("name")
        .ok_or_else(|| Error::Usage("a feature name is required".into()))?
        .clone();
    feature::validate_name(&name)?;
    Ok(name)
}

/// Require the feature's directory to be present, then read its state.
fn require(name: &str) -> Result<State> {
    if !feature::exists(name) {
        return Err(Error::NotFound(name.to_string()));
    }
    registry::read_state(name)
}

/// Run one transition: persist `pending`, run the script, persist `settled`.
/// On script failure `pending` stays in the registry as the interrupted marker.
fn transition(name: &str, pending: State, phase: Phase, settled: State) -> Result<()> {
    registry::write_state(name, pending)?;
    feature::run_phase(name, phase)?;
    registry::write_state(name, settled)?;
    Ok(())
}

fn install(name: &str) -> Result<()> {
    match require(name)? {
        // Already past the install boundary.
        State::Installed | State::Enabling | State::Disabling | State::Enabled => {
            println!("feat: {name} already installed");
            Ok(())
        }
        // Fresh, an interrupted install (retry), or an interrupted uninstall
        // (reinstall) — all resolve by running install.sh to a clean Installed.
        State::NotInstalled | State::Installing | State::Uninstalling => {
            transition(name, State::Installing, Phase::Install, State::Installed)?;
            println!("feat: installed {name}");
            Ok(())
        }
    }
}

fn enable(name: &str) -> Result<()> {
    match require(name)? {
        State::NotInstalled | State::Installing | State::Uninstalling => Err(Error::State(format!(
            "feature {name} is not installed (run `feat install {name}` or `feat add {name}`)"
        ))),
        State::Enabled => {
            println!("feat: {name} already enabled");
            Ok(())
        }
        // Installed, or an interrupted enable/disable — run enable.sh to Enabled.
        State::Installed | State::Enabling | State::Disabling => {
            transition(name, State::Enabling, Phase::Enable, State::Enabled)?;
            println!("feat: enabled {name}");
            Ok(())
        }
    }
}

fn disable(name: &str) -> Result<()> {
    match require(name)? {
        // On, or an interrupted enable/disable — run disable.sh back to Installed.
        State::Enabled | State::Enabling | State::Disabling => {
            transition(name, State::Disabling, Phase::Disable, State::Installed)?;
            println!("feat: disabled {name}");
            Ok(())
        }
        _ => {
            println!("feat: {name} is not enabled");
            Ok(())
        }
    }
}

fn add(name: &str) -> Result<()> {
    install(name)?;
    enable(name)
}

/// remove / uninstall: peel back whatever is set up. Disable first if it is on
/// (you cannot uninstall while enabled), then uninstall. Pending states are
/// resolved toward NotInstalled.
fn remove(name: &str) -> Result<()> {
    let state = require(name)?;
    if state == State::NotInstalled {
        println!("feat: {name} is not installed");
        return Ok(());
    }
    if matches!(state, State::Enabled | State::Enabling | State::Disabling) {
        transition(name, State::Disabling, Phase::Disable, State::Installed)?;
        println!("feat: disabled {name}");
    }
    transition(name, State::Uninstalling, Phase::Uninstall, State::NotInstalled)?;
    println!("feat: uninstalled {name}");
    Ok(())
}

fn list() -> Result<()> {
    let names = feature::list()?;
    if names.is_empty() {
        println!("no features available");
        return Ok(());
    }
    for name in names {
        let state = registry::read_state(&name)?;
        println!("{name}\t{}", state.label());
    }
    Ok(())
}
