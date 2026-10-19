// `reg new <key>` — create a key.

use clap::ArgMatches;
use serde_json::json;

use peios::registry::{CreateFlags, Disposition, Key, KeyAccess};

use crate::cmd;
use crate::error::{Error, Result};
use crate::render::{CmdOutput, Lines};
use crate::settings::Settings;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = path.display(set.sep);

    let mut flags = CreateFlags::empty();
    if m.get_flag("volatile") {
        flags |= CreateFlags::VOLATILE;
    }

    if m.get_flag("parents") {
        cmd::create_ancestors(&path, set.layer_arg(), &set)?;
    }

    let (_key, disp) = Key::create(
        None,
        &path.to_abi(),
        KeyAccess::WRITE,
        flags,
        set.layer_arg(),
        None,
    )
    .map_err(|e| Error::from_peios("create key", &target, e))?;

    let created = disp == Disposition::CreatedNew;
    let mut lines = Lines::new();
    if !set.quiet {
        lines.plain(format!(
            "{} {target}   (layer: {})",
            if created { "created" } else { "exists" },
            set.layer_label()
        ));
    }
    cmd::emit(
        &CmdOutput {
            human: lines,
            json: json!({ "key": target, "created": created, "layer": set.layer_label() }),
        },
        &set,
    );
    Ok(())
}
