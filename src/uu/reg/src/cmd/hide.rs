// `reg hide` / `reg unhide` — mask (or unmask) a key's existence in a layer.
//
// Hiding installs a HIDDEN path entry in the target layer; unhiding removes
// that layer's path entry (`delete_key` on the layer), letting the key reappear.

use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{KeyAccess, OpenFlags};
use serde_json::json;

pub fn run(m: &ArgMatches, hide: bool) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = path.display(set.sep);
    let key = cmd::open(&path, KeyAccess::DELETE, OpenFlags::empty(), &set)?;

    if hide {
        key.hide_key(set.layer_arg(), None)
            .map_err(|e| Error::from_peios("hide key", &target, e))?;
    } else {
        key.delete_key(set.layer_arg(), None)
            .map_err(|e| Error::from_peios("unhide key", &target, e))?;
    }
    cmd::report(
        &set,
        json!({ "key": target, "hidden": hide, "layer": set.layer_label() }),
        &format!(
            "{} {} (layer: {})",
            if hide { "hid" } else { "unhid" },
            target,
            set.layer_label()
        ),
    );
    Ok(())
}
