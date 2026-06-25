// `reg del <key> [value]` — delete a value, or a key (optionally recursive).

use crate::addr::{display_value_name, KeyPath, ValueTarget};
use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{KeyAccess, OpenFlags};
use serde_json::json;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = path.display(set.sep);
    let recursive = m.get_flag("recursive");

    match cmd::value_target(m) {
        ValueTarget::Value(name) => {
            let key = cmd::open(&path, KeyAccess::SET_VALUE, OpenFlags::empty(), &set)?;
            key.delete_value(&name, set.layer_arg(), None)
                .map_err(|e| Error::from_peios("delete value", &target, e))?;
            report(&set, json!({ "deleted_value": display_value_name(&name), "key": target }),
                   &format!("deleted value {} from {}", display_value_name(&name), target));
            Ok(())
        }
        ValueTarget::Key if recursive => {
            if !set.confirm(&format!("Recursively delete key {target} and all its contents?"))? {
                return Err(Error::Usage("aborted".into()));
            }
            let n = purge(&path, &set)?;
            report(&set, json!({ "deleted_key": target, "recursive": true, "keys_removed": n }),
                   &format!("deleted {target} ({n} keys)"));
            Ok(())
        }
        ValueTarget::Key => {
            let key = cmd::open(&path, KeyAccess::DELETE, OpenFlags::empty(), &set)?;
            key.delete_key(set.layer_arg(), None)
                .map_err(|e| Error::from_peios("delete key", &target, e))?;
            report(&set, json!({ "deleted_key": target }), &format!("deleted {target}"));
            Ok(())
        }
    }
}

/// Recursively delete `path` and everything under it. Returns the key count.
/// Children are collected before deletion (we don't delete mid-enumeration).
fn purge(path: &KeyPath, set: &Settings) -> Result<u64> {
    let target = path.display(set.sep);
    let key = cmd::open(
        path,
        KeyAccess::DELETE | KeyAccess::ENUMERATE_SUB_KEYS,
        OpenFlags::empty(),
        set,
    )?;
    let mut children = Vec::new();
    for sk in key.subkeys(None) {
        let sk = sk.map_err(|e| Error::from_peios("enumerate subkeys", &target, e))?;
        children.push(String::from_utf8_lossy(&sk.name).into_owned());
    }
    let mut count = 0;
    for c in children {
        count += purge(&path.child(&c), set)?;
    }
    key.delete_key(set.layer_arg(), None)
        .map_err(|e| Error::from_peios("delete key", &target, e))?;
    Ok(count + 1)
}

fn report(set: &Settings, json: serde_json::Value, human: &str) {
    if set.json {
        println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
    } else if !set.quiet {
        println!("{human}");
    }
}
