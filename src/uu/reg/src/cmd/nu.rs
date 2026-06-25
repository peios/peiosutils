// `reg new <key>` — create a key.

use crate::cmd;
use crate::error::{Error, Result};
use crate::render::{CmdOutput, Lines};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{CreateFlags, Disposition, Key, KeyAccess};
use serde_json::json;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = path.display(set.sep);

    let mut flags = CreateFlags::empty();
    if m.get_flag("volatile") {
        flags |= CreateFlags::VOLATILE;
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
            "{} {}   (layer: {})",
            if created { "created" } else { "exists" },
            target,
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
