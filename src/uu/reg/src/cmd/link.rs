// `reg link <key> <target>` — create a symlink key.
//
// A symlink key carries an immutable symlink flag (set at creation) plus a
// default REG_LINK value holding the absolute target path.

use crate::addr::KeyPath;
use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{CreateFlags, Key, KeyAccess, ValueType};
use serde_json::json;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = path.display(set.sep);
    let dest_raw = m
        .get_one::<String>("target")
        .ok_or_else(|| Error::Usage("missing symlink target".into()))?;
    let dest = KeyPath::parse(dest_raw)?.to_abi();

    let (key, _disp) = Key::create(
        None,
        &path.to_abi(),
        KeyAccess::CREATE_LINK | KeyAccess::SET_VALUE,
        CreateFlags::CREATE_LINK,
        set.layer_arg(),
        None,
    )
    .map_err(|e| Error::from_peios("create symlink key", &target, e))?;

    // Store the target as the default REG_LINK value (UTF-8 + NUL terminator).
    let mut bytes = dest.as_bytes().to_vec();
    bytes.push(0);
    let mut sv = key.set_value(&[], ValueType::LINK, &bytes);
    if let Some(l) = set.layer_arg() {
        sv.layer(l);
    }
    sv.call()
        .map_err(|e| Error::from_peios("set link target", &target, e))?;

    cmd::report(
        &set,
        json!({ "link": target, "target": dest, "layer": set.layer_label() }),
        &format!("linked {target} -> {dest} (layer: {})", set.layer_label()),
    );
    Ok(())
}
