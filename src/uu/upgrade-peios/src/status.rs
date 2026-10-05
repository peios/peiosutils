// --status: which release this is, what of it is waiting for the next
// boot, and whether the caller may upgrade it — for a person, or as JSON
// for a program such as Upgrade Peios.

use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::json;

use crate::{edition_of, unquote, Error, Result, AUTOAPPLY_DIR};

/// peipkg's state: every upgrade takes its lock and writes its database.
const PEIPKG_STATE: [&str; 2] = ["var/state/peipkg/lock", "var/state/peipkg/db.sqlite"];

pub fn show(root: &Path, live: bool, as_json: bool) -> Result<()> {
    let path = root.join("usr/lib/os-release");
    let text = fs::read_to_string(&path).map_err(|e| Error::NoRelease(format!("{}: {e}", path.display())))?;
    let edition = edition_of(&text, &path)?;
    let field = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
            .map(unquote)
    };
    let version = installed_version(root, live, &edition);
    let queued = queued(root);
    let may = may_upgrade(root);
    if as_json {
        println!(
            "{}",
            json!({
                "edition": edition,
                "name": field("PRETTY_NAME"),
                "version_id": field("VERSION_ID"),
                "variant": field("VARIANT"),
                "version": version,
                "queued_seeds": queued,
                "may_upgrade": may,
            })
        );
        return Ok(());
    }
    println!("{}", field("PRETTY_NAME").unwrap_or_else(|| edition.clone()));
    match &version {
        Some(v) => println!("Edition:       {edition} {v}"),
        None => println!("Edition:       {edition} (its version can't be read)"),
    }
    match &queued {
        Some(names) if names.is_empty() => println!("Queued seeds:  none"),
        Some(names) => println!("Queued seeds:  {} (applied at the next boot)", names.join(", ")),
        None => println!("Queued seeds:  can't be read"),
    }
    println!("May upgrade:   {}", if may { "yes" } else { "no" });
    Ok(())
}

/// The installed edition package's version, from peipkg; `None` where its
/// records can't be read.
fn installed_version(root: &Path, live: bool, edition: &str) -> Option<String> {
    let mut cmd = Command::new("peipkg");
    if !live {
        cmd.arg("--root").arg(root);
    }
    let out = cmd.args(["info", "--json", edition]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let info: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    info.get("version").and_then(serde_json::Value::as_str).map(str::to_string)
}

/// The seeds waiting in the queue for the next boot, by name, sorted; `None`
/// where the queue can't be read.
fn queued(root: &Path) -> Option<Vec<String>> {
    let entries = match fs::read_dir(root.join(AUTOAPPLY_DIR)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Some(Vec::new()),
        Err(_) => return None,
    };
    let mut names: Vec<String> = entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| entry.file_name().to_str().and_then(|n| n.strip_suffix(".reg")).map(str::to_string))
        .collect();
    names.sort();
    Some(names)
}

/// Whether the caller may upgrade, asked of the system rather than guessed
/// from who they are: may they open peipkg's lock and database to write,
/// and write in the seed queue (or, where it isn't made yet, where it would
/// be made)? Nothing is written. Whether every seed applies is the
/// registry's to say when it does.
fn may_upgrade(root: &Path) -> bool {
    for state in PEIPKG_STATE {
        match OpenOptions::new().read(true).write(true).open(root.join(state)) {
            Err(e) if e.kind() == ErrorKind::PermissionDenied => return false,
            // Not there yet: peipkg makes it, and is the one to refuse.
            _ => {}
        }
    }
    let queue = root.join(AUTOAPPLY_DIR);
    let dir = if queue.exists() { queue } else { queue.parent().map(Path::to_path_buf).unwrap_or(queue) };
    let Ok(dir) = CString::new(dir.as_os_str().as_bytes()) else { return false };
    // SAFETY: a NUL-terminated path; access() reads it and nothing else.
    unsafe { libc::access(dir.as_ptr(), libc::W_OK) == 0 }
}
