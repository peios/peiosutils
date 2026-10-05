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
//
// A change is planned from the state it starts in before anything runs, so a
// driven caller is told how many scripts to expect.

use clap::ArgMatches;
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::feature::{self, Metadata, Phase};
use crate::registry::{self, State};
use crate::report::Report;

pub fn dispatch(matches: &ArgMatches, report: Report) -> Result<()> {
    let (name, m) = matches
        .subcommand()
        .ok_or_else(|| Error::Usage("a subcommand is required".into()))?;
    match name {
        "list" | "info" if report.driven() => Err(Error::Usage(format!(
            "--driven reports a change; {name} answers a program with --json"
        ))),
        "list" => list(m.get_flag("json")),
        "info" => info(&arg_name(m)?, m.get_flag("json")),
        "install" => change(&arg_name(m)?, Verb::Install, report),
        "enable" => change(&arg_name(m)?, Verb::Enable, report),
        "disable" => change(&arg_name(m)?, Verb::Disable, report),
        "add" => change(&arg_name(m)?, Verb::Add, report),
        "remove" | "uninstall" => change(&arg_name(m)?, Verb::Remove, report),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verb {
    Install,
    Enable,
    Disable,
    Add,
    Remove,
}

/// One transition: the state written before its script, the script, the
/// state written after, and what feat says once it is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step {
    pending: State,
    phase: Phase,
    settled: State,
    said: &'static str,
}

const INSTALL: Step = Step {
    pending: State::Installing,
    phase: Phase::Install,
    settled: State::Installed,
    said: "installed",
};
const ENABLE: Step = Step {
    pending: State::Enabling,
    phase: Phase::Enable,
    settled: State::Enabled,
    said: "enabled",
};
const DISABLE: Step = Step {
    pending: State::Disabling,
    phase: Phase::Disable,
    settled: State::Installed,
    said: "disabled",
};
const UNINSTALL: Step = Step {
    pending: State::Uninstalling,
    phase: Phase::Uninstall,
    settled: State::NotInstalled,
    said: "uninstalled",
};

/// What a change does, in order: run a step, or say why a part of it needs
/// nothing doing.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Run(Step),
    Say(String),
}

/// Plan `verb` for a feature in `state`: its actions, and the state they
/// leave it in.
fn plan(name: &str, verb: Verb, state: State) -> Result<(Vec<Action>, State)> {
    use State::*;
    let say = |line: String| Ok((vec![Action::Say(line)], state));
    let run = |steps: &[Step]| {
        let settled = steps.last().map_or(state, |s| s.settled);
        Ok((steps.iter().copied().map(Action::Run).collect(), settled))
    };
    match verb {
        Verb::Install => match state {
            // Already past the install boundary.
            Installed | Enabling | Disabling | Enabled => say(format!("{name} already installed")),
            // Fresh, an interrupted install (retry), or an interrupted
            // uninstall (reinstall) — all resolve by running install.sh to a
            // clean Installed.
            NotInstalled | Installing | Uninstalling => run(&[INSTALL]),
        },
        Verb::Enable => match state {
            NotInstalled | Installing | Uninstalling => Err(Error::State(format!(
                "feature {name} is not installed (run `feat install {name}` or `feat add {name}`)"
            ))),
            Enabled => say(format!("{name} already enabled")),
            // Installed, or an interrupted enable/disable — run enable.sh to Enabled.
            Installed | Enabling | Disabling => run(&[ENABLE]),
        },
        Verb::Disable => match state {
            // On, or an interrupted enable/disable — run disable.sh back to Installed.
            Enabled | Enabling | Disabling => run(&[DISABLE]),
            _ => say(format!("{name} is not enabled")),
        },
        Verb::Add => {
            let (mut actions, installed) = plan(name, Verb::Install, state)?;
            let (enable, enabled) = plan(name, Verb::Enable, installed)?;
            actions.extend(enable);
            Ok((actions, enabled))
        }
        // Peel back whatever is set up: disable first if it is on (you cannot
        // uninstall while enabled), then uninstall. Pending states are
        // resolved toward NotInstalled.
        Verb::Remove => match state {
            NotInstalled => say(format!("{name} is not installed")),
            Enabled | Enabling | Disabling => run(&[DISABLE, UNINSTALL]),
            Installed | Installing | Uninstalling => run(&[UNINSTALL]),
        },
    }
}

fn change(name: &str, verb: Verb, report: Report) -> Result<()> {
    let (actions, _) = plan(name, verb, require(name)?)?;
    let steps = actions.iter().filter(|a| matches!(a, Action::Run(_))).count();
    let mut step = 0;
    let mut last = String::new();
    for action in actions {
        last = match action {
            Action::Say(line) => line,
            Action::Run(s) => {
                step += 1;
                report.progress(s.phase, step, steps);
                transition(name, s, report)?;
                format!("{} {name}", s.said)
            }
        };
        report.said(&last);
    }
    if report.driven() {
        report.done(&last, registry::read_state(name)?);
    }
    Ok(())
}

/// Run one transition: persist `pending`, run the script, persist `settled`.
/// On script failure `pending` stays in the registry as the interrupted marker.
fn transition(name: &str, step: Step, report: Report) -> Result<()> {
    registry::write_state(name, step.pending)?;
    feature::run_phase(name, step.phase, report)?;
    registry::write_state(name, step.settled)?;
    Ok(())
}

/// Everything known about one feature, for `list` and `info`.
struct Feature {
    name: String,
    state: State,
    /// Its definition directory is present. A feature whose package was
    /// removed while it was set up has a state and no definition.
    defined: bool,
    phases: Vec<Phase>,
    metadata: std::result::Result<Metadata, String>,
}

impl Feature {
    fn read(name: &str) -> Result<Self> {
        let defined = feature::exists(name);
        Ok(Self {
            name: name.to_string(),
            state: registry::read_state(name)?,
            defined,
            phases: if defined { feature::phases(name) } else { Vec::new() },
            metadata: if defined { feature::metadata(name) } else { Ok(Metadata::default()) },
        })
    }

    fn json(&self) -> Result<Value> {
        let metadata = self.metadata.clone().unwrap_or_default();
        let mut v = json!({
            "name": self.name,
            "title": metadata.title,
            "description": metadata.description,
            "state": self.state.label(),
            "defined": self.defined,
            "phases": self.phases.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
            "may_change": registry::may_change(&self.name)?,
        });
        if let Err(problem) = &self.metadata {
            v["metadata_problem"] = json!(problem);
        }
        Ok(v)
    }
}

/// Every feature: each one defined, then each one only the registry still
/// remembers as set up.
fn every() -> Result<Vec<Feature>> {
    let mut features = Vec::new();
    for name in feature::list()? {
        features.push(Feature::read(&name)?);
    }
    for name in registry::recorded()? {
        if feature::validate_name(&name).is_err() || feature::exists(&name) {
            continue;
        }
        let gone = Feature::read(&name)?;
        if gone.state != State::NotInstalled {
            features.push(gone);
        }
    }
    Ok(features)
}

fn list(json: bool) -> Result<()> {
    let features = every()?;
    if json {
        let all = features.iter().map(Feature::json).collect::<Result<Vec<_>>>()?;
        println!("{}", Value::Array(all));
        return Ok(());
    }
    if features.is_empty() {
        println!("no features available");
        return Ok(());
    }
    for f in features {
        if f.defined {
            println!("{}\t{}", f.name, f.state.label());
        } else {
            println!("{}\t{}\tno definition", f.name, f.state.label());
        }
    }
    Ok(())
}

fn info(name: &str, json: bool) -> Result<()> {
    let f = Feature::read(name)?;
    if !f.defined && f.state == State::NotInstalled {
        return Err(Error::NotFound(name.to_string()));
    }
    if json {
        println!("{}", f.json()?);
        return Ok(());
    }
    let metadata = match &f.metadata {
        Ok(m) => m.clone(),
        Err(problem) => {
            eprintln!("feat: warning: {name}: {problem}");
            Metadata::default()
        }
    };
    println!("Name:        {}", f.name);
    if let Some(title) = &metadata.title {
        println!("Title:       {title}");
    }
    println!("State:       {}", f.state.label());
    if f.defined {
        let scripts = f.phases.iter().map(|p| p.as_str()).collect::<Vec<_>>();
        println!("Scripts:     {}", if scripts.is_empty() { "none".to_string() } else { scripts.join(", ") });
    } else {
        println!("Definition:  missing (no {})", feature::feature_dir(name).display());
    }
    if let Some(description) = &metadata.description {
        // Paragraphs are already a blank line apart.
        println!("\n{description}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{plan, Action, Verb, DISABLE, ENABLE, INSTALL, UNINSTALL};
    use crate::registry::State;

    fn runs(verb: Verb, state: State) -> Vec<Action> {
        plan("f", verb, state).unwrap().0
    }

    #[test]
    fn add_installs_then_enables_and_resumes_either_half() {
        use Action::*;
        assert_eq!(runs(Verb::Add, State::NotInstalled), [Run(INSTALL), Run(ENABLE)]);
        assert_eq!(runs(Verb::Add, State::Installing), [Run(INSTALL), Run(ENABLE)]);
        assert_eq!(
            runs(Verb::Add, State::Enabling),
            [Say("f already installed".into()), Run(ENABLE)]
        );
        assert_eq!(
            runs(Verb::Add, State::Enabled),
            [Say("f already installed".into()), Say("f already enabled".into())]
        );
    }

    #[test]
    fn remove_disables_first_only_when_on() {
        use Action::*;
        assert_eq!(runs(Verb::Remove, State::Enabled), [Run(DISABLE), Run(UNINSTALL)]);
        assert_eq!(runs(Verb::Remove, State::Disabling), [Run(DISABLE), Run(UNINSTALL)]);
        assert_eq!(runs(Verb::Remove, State::Installed), [Run(UNINSTALL)]);
        assert_eq!(runs(Verb::Remove, State::NotInstalled), [Say("f is not installed".into())]);
    }

    #[test]
    fn a_plan_says_where_it_leaves_the_feature() {
        assert_eq!(plan("f", Verb::Add, State::NotInstalled).unwrap().1, State::Enabled);
        assert_eq!(plan("f", Verb::Remove, State::Enabled).unwrap().1, State::NotInstalled);
        assert_eq!(plan("f", Verb::Disable, State::Installed).unwrap().1, State::Installed);
    }

    #[test]
    fn enabling_what_is_not_installed_is_refused() {
        for state in [State::NotInstalled, State::Installing, State::Uninstalling] {
            assert!(plan("f", Verb::Enable, state).is_err());
        }
    }
}
