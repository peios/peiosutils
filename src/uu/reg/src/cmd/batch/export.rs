// `reg export <key> [file]` — dump a subtree to the batch format.

use super::{Document, KeyEntry, ValueEntry};
use crate::addr::KeyPath;
use crate::cmd;
use crate::error::{Error, Result};
use crate::literal;
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{KeyAccess, OpenFlags};
use std::io::Write;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let file = m.get_one::<String>("file").map(String::as_str);

    let mut doc = Document::default();
    collect(&path, &set, &mut doc)?;

    let rendered = if set.json {
        serde_json::to_string_pretty(&doc).map_err(|e| Error::InvalidSpec(e.to_string()))?
    } else {
        render_text(&doc)
    };

    match file {
        None | Some("-") => {
            println!("{rendered}");
        }
        Some(f) => {
            let mut out = std::fs::File::create(f).map_err(|e| Error::Syscall {
                op: "create export file",
                errno: e.raw_os_error().unwrap_or(5),
                detail: Some(f.to_string()),
            })?;
            writeln!(out, "{rendered}").map_err(|e| Error::Syscall {
                op: "write export file",
                errno: e.raw_os_error().unwrap_or(5),
                detail: Some(f.to_string()),
            })?;
        }
    }
    Ok(())
}

/// Recursively collect keys + values into `doc` (depth-first, stable order).
fn collect(path: &KeyPath, set: &Settings, doc: &mut Document) -> Result<()> {
    let target = path.display(set.sep);
    let key = cmd::open(path, KeyAccess::READ, OpenFlags::empty(), set)?;

    let mut values = Vec::new();
    if let Ok(records) = key.query_values_batch(None) {
        for r in &records {
            let name = if r.name.is_empty() {
                "@".to_string()
            } else {
                String::from_utf8_lossy(&r.name).into_owned()
            };
            values.push(ValueEntry {
                name,
                ty: literal::type_keyword(r.ty),
                data: literal::format_json(r.ty, &r.data)["data"].clone(),
            });
        }
    }
    doc.keys.push(KeyEntry {
        path: path.to_abi(),
        values,
    });

    let children: Vec<String> = key
        .subkeys(None)
        .map(|sk| sk.map(|s| String::from_utf8_lossy(&s.name).into_owned()))
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| Error::from_peios("enumerate subkeys", &target, e))?;
    for c in children {
        collect(&path.child(&c), set, doc)?;
    }
    Ok(())
}

/// Render a document in the §6 text format.
fn render_text(doc: &Document) -> String {
    let mut out = String::new();
    for k in &doc.keys {
        out.push_str(&format!("[key {}]\n", k.path));
        for v in &k.values {
            // Reconstruct bytes to produce an exact, explicit literal token.
            if let Ok((ty, bytes)) = super::apply::value_bytes(&v.ty, &v.data) {
                out.push_str(&format!("  {} = {}\n", v.name, literal::to_token(ty, &bytes)));
            }
        }
        out.push('\n');
    }
    out
}
