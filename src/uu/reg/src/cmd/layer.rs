// `reg layer …` — manage layers.
//
// Layers are configured through their metadata keys under
// `Machine\System\Registry\Layers\<name>\` (LCS TRM §5.3.3), which
// `peios::registry::layers` reads and writes: a `Precedence` (DWORD), an
// `Enabled` (DWORD 0/1), and an informational `Owner` (a binary SID).

use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::layers::{self, LAYERS};
use peios::security::Sid;
use serde_json::json;

pub fn run(m: &ArgMatches) -> Result<()> {
    let (name, sub) = m
        .subcommand()
        .ok_or_else(|| Error::Usage("layer: subcommand required (ls|new|set|del)".into()))?;
    match name {
        "ls" => ls(sub),
        "new" => new(sub),
        "set" => set(sub),
        "del" => del(sub),
        other => Err(Error::Usage(format!("unknown layer subcommand: {other}"))),
    }
}

fn ls(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let rows = layers::list().map_err(|e| Error::from_peios("enumerate layers", LAYERS, e))?;

    if set.json {
        let arr: Vec<_> = rows
            .iter()
            .map(|l| {
                json!({
                    "name": l.name,
                    "precedence": l.precedence,
                    "enabled": l.enabled,
                    "owner": l.owner.as_ref().map(ToString::to_string),
                    "malformed": l.malformed,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json!({ "layers": arr })).unwrap_or_default());
    } else {
        for l in &rows {
            println!(
                "{:<24} prec={:<6} {}{}{}",
                l.name,
                l.precedence,
                if l.enabled { "enabled " } else { "disabled" },
                l.owner.as_ref().map(|s| format!("  owner={s}")).unwrap_or_default(),
                if l.malformed { "  (malformed metadata)" } else { "" },
            );
        }
    }
    Ok(())
}

fn new(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let name = m.get_one::<String>("name").unwrap();
    let precedence = m.get_one::<u32>("precedence").copied().unwrap_or(0);
    let enabled = !m.get_flag("disabled");
    let owner = m.get_one::<String>("owner").map(|o| sid(o)).transpose()?;

    layers::create(name, precedence, enabled).map_err(|e| Error::from_peios("create layer", name, e))?;
    if let Some(owner) = owner {
        layers::set_owner(name, &owner).map_err(|e| Error::from_peios("write layer metadata", name, e))?;
    }
    cmd::report(
        &set,
        json!({ "layer": name, "precedence": precedence, "enabled": enabled }),
        &format!("created layer {name} (precedence {precedence}, {})",
                 if enabled { "enabled" } else { "disabled" }),
    );
    Ok(())
}

fn set(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let name = m.get_one::<String>("name").unwrap();
    let failed = |e| Error::from_peios("write layer metadata", name, e);

    if let Some(p) = m.get_one::<u32>("precedence").copied() {
        layers::set_precedence(name, p).map_err(failed)?;
    }
    if m.get_flag("enable") {
        layers::set_enabled(name, true).map_err(failed)?;
    }
    if m.get_flag("disable") {
        layers::set_enabled(name, false).map_err(failed)?;
    }
    if let Some(o) = m.get_one::<String>("owner") {
        layers::set_owner(name, &sid(o)?).map_err(failed)?;
    }
    cmd::report(&set, json!({ "layer": name, "updated": true }), &format!("updated layer {name}"));
    Ok(())
}

fn del(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let name = m.get_one::<String>("name").unwrap();
    if !set.confirm(&format!("Delete layer {name} and all its entries?"))? {
        return Err(Error::Usage("aborted".into()));
    }
    layers::delete(name).map_err(|e| Error::from_peios("delete layer", name, e))?;
    cmd::report(&set, json!({ "layer": name, "deleted": true }), &format!("deleted layer {name}"));
    Ok(())
}

/// The owner as a SID, which the registry keeps in binary.
fn sid(text: &str) -> Result<Sid> {
    text.parse().map_err(|_| Error::InvalidSpec(format!("{text:?}: not a SID")))
}
