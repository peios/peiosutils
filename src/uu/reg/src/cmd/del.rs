// `reg del <key> [value]` — delete a value, or a key (optionally recursive).

use crate::addr::{display_value_name, KeyPath, ValueTarget};
use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{KeyAccess, OpenFlags, Transaction};
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

/// Delete `path` and everything under it, all or nothing, in one
/// transaction. Links under it are deleted, not followed. Returns the key
/// count.
fn purge(path: &KeyPath, set: &Settings) -> Result<u64> {
    let target = path.display(set.sep);
    let key = cmd::open(
        path,
        KeyAccess::DELETE | KeyAccess::ENUMERATE_SUB_KEYS,
        OpenFlags::empty(),
        set,
    )?;
    let txn = Transaction::begin().map_err(|e| Error::from_peios("begin transaction", &target, e))?;
    let count = key
        .delete_tree(set.layer_arg(), Some(&txn))
        .map_err(|e| Error::from_peios("delete key", &target, e))?;
    txn.commit().map_err(|e| Error::from_peios("commit transaction", &target, e))?;
    Ok(count)
}

fn report(set: &Settings, json: serde_json::Value, human: &str) {
    if set.json {
        println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
    } else if !set.quiet {
        println!("{human}");
    }
}
