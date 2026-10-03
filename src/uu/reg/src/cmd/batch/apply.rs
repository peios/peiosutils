// `reg apply <file>` — apply a batch of operations in one transaction.

use super::Document;
use crate::addr::KeyPath;
use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use std::io::Read;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let once_delete = m.get_flag("once-delete");

    if let Some(dir) = m.get_one::<String>("dir") {
        return run_dir(dir, once_delete, &set);
    }

    let file = m
        .get_one::<String>("file")
        .ok_or_else(|| Error::Usage("missing batch file".into()))?;
    let applied = apply_file(file, once_delete, &set)?;
    cmd::report(
        &set,
        serde_json::json!({ "applied": applied }),
        &format!("applied {applied} keys"),
    );
    Ok(())
}

/// Apply every batch file in `dir` (sorted), each as its own transaction. A
/// missing or empty directory is not an error — it applies nothing and reports
/// zero, so a caller (e.g. peinit's first-boot seed sweep) can run it
/// unconditionally. With `once_delete`, each file is unlinked after it applies
/// successfully. A file that fails to apply is reported and skipped (left in
/// place); the sweep continues — one bad seed never blocks the rest.
fn run_dir(dir: &str, once_delete: bool, set: &Settings) -> Result<()> {
    let mut files = list_batch_files(dir)?;
    files.sort();

    let mut applied_files = 0usize;
    let mut applied_keys = 0usize;
    let mut failed = 0usize;
    for file in &files {
        let path = file.to_string_lossy().into_owned();
        match apply_file(&path, once_delete, set) {
            Ok(keys) => {
                applied_files += 1;
                applied_keys += keys;
            }
            Err(e) => {
                failed += 1;
                eprintln!("reg: apply {path}: {e}");
            }
        }
    }

    cmd::report(
        set,
        serde_json::json!({
            "applied_files": applied_files,
            "applied_keys": applied_keys,
            "failed": failed,
        }),
        &format!("applied {applied_files} file(s), {applied_keys} key(s); {failed} failed"),
    );
    if failed > 0 {
        return Err(Error::Usage(format!("{failed} batch file(s) failed to apply")));
    }
    Ok(())
}

/// List regular files in `dir` that look like batch files. A missing directory
/// yields an empty list (not an error); other I/O errors propagate.
fn list_batch_files(dir: &str) -> Result<Vec<std::path::PathBuf>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(Error::Syscall {
                op: "read seed directory",
                errno: e.raw_os_error().unwrap_or(5),
                detail: Some(dir.to_string()),
            });
        }
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| Error::Syscall {
            op: "read seed directory entry",
            errno: e.raw_os_error().unwrap_or(5),
            detail: Some(dir.to_string()),
        })?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        files.push(entry.path());
    }
    Ok(files)
}

/// Apply a single batch file, returning the number of keys applied. With
/// `once_delete`, unlink the file after a successful commit.
fn apply_file(file: &str, once_delete: bool, set: &Settings) -> Result<usize> {
    let text = read_input(file)?;
    let trimmed = text.trim_start();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        // Text batch input: the §6.1 escaping grammar is not finalised, so the
        // text *parser* is intentionally not shipped yet. JSON is exact today.
        return Err(Error::Usage(
            "text-format apply is not implemented yet (pending the §6.1 escaping \
             grammar); supply a JSON batch, or use `reg export --json` to produce one"
                .into(),
        ));
    }

    let doc: Document = serde_json::from_str(&text)
        .map_err(|e| Error::InvalidSpec(format!("batch JSON: {e}")))?;
    let applied = apply_doc(&doc, set)?;

    if once_delete && file != "-" {
        std::fs::remove_file(file).map_err(|e| Error::Syscall {
            op: "delete applied batch file",
            errno: e.raw_os_error().unwrap_or(5),
            detail: Some(file.to_string()),
        })?;
    }
    Ok(applied)
}

fn read_input(file: &str) -> Result<String> {
    if file == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).map_err(|e| Error::Syscall {
            op: "read stdin",
            errno: e.raw_os_error().unwrap_or(5),
            detail: None,
        })?;
        Ok(s)
    } else {
        std::fs::read_to_string(file).map_err(|e| Error::Syscall {
            op: "read batch file",
            errno: e.raw_os_error().unwrap_or(5),
            detail: Some(file.to_string()),
        })
    }
}

/// Apply the document atomically: every key create and value set is enlisted in
/// one transaction, committed at the end (all-or-nothing). Hive-scoped — a
/// cross-hive document surfaces EXDEV from the kernel. Returns the number of
/// keys applied; reporting is left to the caller (single-file vs `--dir` sweep
/// report differently).
fn apply_doc(doc: &Document, set: &Settings) -> Result<usize> {
    for entry in &doc.keys {
        KeyPath::parse(&entry.path)?;
    }
    Ok(libreg::apply(doc, set.layer_arg())?)
}
