// `reg apply <file>` — apply a batch of operations in one transaction.

use super::Document;
use crate::addr::KeyPath;
use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{CreateFlags, Key, KeyAccess, Transaction, ValueType};
use serde_json::Value as Json;
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
    let txn = Transaction::begin().map_err(|e| Error::from_peios("begin transaction", "", e))?;
    // Hold key fds open until after commit so enlisted ops stay valid.
    let mut keys = Vec::new();

    for entry in &doc.keys {
        let path = KeyPath::parse(&entry.path)?;
        let (key, _) = Key::create(
            None,
            &path.to_abi(),
            KeyAccess::WRITE | KeyAccess::SET_VALUE,
            CreateFlags::empty(),
            set.layer_arg(),
            Some(&txn),
        )
        .map_err(|e| Error::from_peios("create key", &entry.path, e))?;

        for v in &entry.values {
            let name = if v.name == "@" { Vec::new() } else { v.name.as_bytes().to_vec() };
            let (ty, bytes) = value_bytes(&v.ty, &v.data)?;
            let mut sv = key.set_value(&name, ty, &bytes);
            if let Some(l) = set.layer_arg() {
                sv.layer(l);
            }
            sv.in_txn(&txn);
            sv.call()
                .map_err(|e| Error::from_peios("set value", &entry.path, e))?;
        }
        keys.push(key);
    }

    txn.commit().map_err(|e| Error::from_peios("commit transaction", "", e))?;
    drop(keys);
    Ok(doc.keys.len())
}

/// Decode a `(type-keyword, JSON data)` pair into `(ValueType, bytes)`, the
/// inverse of [`crate::literal::format_json`]. Shared with `export`'s text
/// renderer.
pub fn value_bytes(ty: &str, data: &Json) -> Result<(ValueType, Vec<u8>)> {
    let bytes = match ty {
        "sz" | "expand" | "link" => {
            let mut b = json_str(data, ty)?.into_bytes();
            b.push(0);
            b
        }
        "dword" => json_u64(data, ty)?
            .try_into()
            .map(|v: u32| v.to_le_bytes().to_vec())
            .map_err(|_| Error::InvalidSpec(format!("{ty}: value does not fit u32")))?,
        "dword-be" => json_u64(data, ty)?
            .try_into()
            .map(|v: u32| v.to_be_bytes().to_vec())
            .map_err(|_| Error::InvalidSpec(format!("{ty}: value does not fit u32")))?,
        "qword" => json_u64(data, ty)?.to_le_bytes().to_vec(),
        "multi" => {
            let arr = data
                .as_array()
                .ok_or_else(|| Error::InvalidSpec("multi: expected a JSON array".into()))?;
            let mut out = Vec::new();
            for e in arr {
                out.extend_from_slice(json_str(e, "multi")?.as_bytes());
                out.push(0);
            }
            out.push(0);
            out
        }
        "binary" => decode_hex(&json_str(data, "binary")?)?,
        "none" => Vec::new(),
        "tombstone" => return Ok((ValueType::TOMBSTONE, Vec::new())),
        other => return Err(Error::InvalidSpec(format!("unknown value type: {other}"))),
    };
    let vt = match ty {
        "sz" => ValueType::SZ,
        "expand" => ValueType::EXPAND_SZ,
        "link" => ValueType::LINK,
        "dword" => ValueType::DWORD,
        "dword-be" => ValueType::DWORD_BIG_ENDIAN,
        "qword" => ValueType::QWORD,
        "multi" => ValueType::MULTI_SZ,
        "binary" => ValueType::BINARY,
        "none" => ValueType::NONE,
        _ => unreachable!(),
    };
    Ok((vt, bytes))
}

fn json_str(v: &Json, ty: &str) -> Result<String> {
    v.as_str()
        .map(str::to_string)
        .ok_or_else(|| Error::InvalidSpec(format!("{ty}: expected a JSON string")))
}

fn json_u64(v: &Json, ty: &str) -> Result<u64> {
    v.as_u64()
        .ok_or_else(|| Error::InvalidSpec(format!("{ty}: expected a non-negative integer")))
}

fn decode_hex(s: &str) -> Result<Vec<u8>> {
    let cleaned: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if cleaned.len() % 2 != 0 {
        return Err(Error::InvalidSpec("binary: odd hex length".into()));
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| Error::InvalidSpec("binary: bad hex".into()))
        })
        .collect()
}
