// `reg mask` / `reg unmask` — per-value tombstones and blanket tombstones.

use crate::addr::{display_value_name, ValueTarget};
use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{KeyAccess, OpenFlags, ValueType};
use serde_json::json;

/// `set == true` masks (creates a tombstone); `false` unmasks (clears it).
pub fn run(m: &ArgMatches, set_mask: bool) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = path.display(set.sep);
    let all = m.get_flag("all");
    let key = cmd::open(&path, KeyAccess::SET_VALUE, OpenFlags::empty(), &set)?;

    if all {
        key.blanket_tombstone(set.layer_arg(), set_mask, None)
            .map_err(|e| Error::from_peios("blanket tombstone", &target, e))?;
        report(&set, json!({ "key": target, "blanket": set_mask, "layer": set.layer_label() }),
               &format!("{} blanket tombstone on {} (layer: {})",
                        if set_mask { "set" } else { "cleared" }, target, set.layer_label()));
        return Ok(());
    }

    let name = match cmd::value_target(m) {
        ValueTarget::Value(n) => n,
        ValueTarget::Key => {
            return Err(Error::Usage(
                "mask needs a value name, or --all for a blanket tombstone".into(),
            ))
        }
    };

    if set_mask {
        let mut sv = key.set_value(&name, ValueType::TOMBSTONE, &[]);
        if let Some(l) = set.layer_arg() {
            sv.layer(l);
        }
        sv.call()
            .map_err(|e| Error::from_peios("set tombstone", &target, e))?;
    } else {
        key.delete_value(&name, set.layer_arg(), None)
            .map_err(|e| Error::from_peios("clear tombstone", &target, e))?;
    }
    report(&set,
           json!({ "key": target, "value": display_value_name(&name), "masked": set_mask, "layer": set.layer_label() }),
           &format!("{} tombstone on {} {} (layer: {})",
                    if set_mask { "set" } else { "cleared" },
                    target, display_value_name(&name), set.layer_label()));
    Ok(())
}

fn report(set: &Settings, json: serde_json::Value, human: &str) {
    if set.json {
        println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
    } else if !set.quiet {
        println!("{human}");
    }
}
