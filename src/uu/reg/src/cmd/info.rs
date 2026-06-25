// `reg info <key>` — key metadata.

use crate::cmd;
use crate::error::{Error, Result};
use crate::render::{CmdOutput, Lines};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{KeyAccess, OpenFlags};
use serde_json::json;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let mut flags = OpenFlags::empty();
    if m.get_flag("no-follow") {
        flags |= OpenFlags::OPEN_LINK;
    }
    let target = path.display(set.sep);
    let key = cmd::open(&path, KeyAccess::READ, flags, &set)?;
    let i = key
        .info()
        .map_err(|e| Error::from_peios("query key info", &target, e))?;

    let name = String::from_utf8_lossy(&i.name).into_owned();
    let mut lines = Lines::new();
    lines.kv("path", &target);
    lines.kv("name", &name);
    lines.kv("subkeys", i.subkey_count.to_string());
    lines.kv("values", i.value_count.to_string());
    lines.kv("last_write_ns", i.last_write_time.to_string());
    lines.kv("hive_generation", i.hive_generation.to_string());
    lines.kv("sd_size", i.sd_size.to_string());
    lines.kv("volatile", i.volatile.to_string());
    lines.kv("symlink", i.symlink.to_string());

    let json = json!({
        "path": target,
        "name": name,
        "subkey_count": i.subkey_count,
        "value_count": i.value_count,
        "last_write_time": i.last_write_time,
        "hive_generation": i.hive_generation,
        "max_subkey_name_len": i.max_subkey_name_len,
        "max_value_name_len": i.max_value_name_len,
        "max_value_data_size": i.max_value_data_size,
        "sd_size": i.sd_size,
        "volatile": i.volatile,
        "symlink": i.symlink,
    });
    cmd::emit(&CmdOutput { human: lines, json }, &set);
    Ok(())
}
