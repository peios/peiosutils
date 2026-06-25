// `reg restore <key> <file>` — replace a key + subtree from a snapshot.

use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{KeyAccess, OpenFlags};
use serde_json::json;
use std::os::fd::AsFd;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = path.display(set.sep);
    let file = m
        .get_one::<String>("file")
        .ok_or_else(|| Error::Usage("missing input file".into()))?;

    if !set.confirm(&format!("Replace key {target} and its entire subtree from {file}?"))? {
        return Err(Error::Usage("aborted".into()));
    }

    let key = cmd::open(
        &path,
        KeyAccess::WRITE | KeyAccess::CREATE_SUB_KEY,
        OpenFlags::empty(),
        &set,
    )?;
    let input = std::fs::File::open(file).map_err(|e| Error::Syscall {
        op: "open snapshot file",
        errno: e.raw_os_error().unwrap_or(5),
        detail: Some(file.clone()),
    })?;
    key.restore(input.as_fd())
        .map_err(|e| Error::from_peios("restore", &target, e))?;

    cmd::report(
        &set,
        json!({ "restore": target, "file": file }),
        &format!("restored {target} from {file}"),
    );
    Ok(())
}
