// `reg backup <key> <file>` — binary snapshot of a key + subtree.

use std::os::fd::AsFd;

use clap::ArgMatches;
use serde_json::json;

use peios::registry::{KeyAccess, OpenFlags};

use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = path.display(set.sep);
    let file = m
        .get_one::<String>("file")
        .ok_or_else(|| Error::Usage("missing output file".into()))?;

    let key = cmd::open(&path, KeyAccess::READ, OpenFlags::empty(), &set)?;
    let out = std::fs::File::create(file).map_err(|e| Error::Syscall {
        op: "create backup file",
        errno: e.raw_os_error().unwrap_or(5),
        detail: Some(file.clone()),
    })?;
    key.backup(out.as_fd())
        .map_err(|e| Error::from_peios("backup", &target, e))?;

    cmd::report(
        &set,
        json!({ "backup": target, "file": file }),
        &format!("backed up {target} -> {file}"),
    );
    Ok(())
}
