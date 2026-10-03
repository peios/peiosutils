// `reg export <key> [file]` — dump a subtree to the batch format.

use super::Document;
use crate::cmd;
use crate::error::{Error, Result};
use crate::literal;
use crate::settings::Settings;
use clap::ArgMatches;
use std::io::Write;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let file = m.get_one::<String>("file").map(String::as_str);

    let doc = libreg::export(&path.to_abi())?;

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

/// Render a document in the §6 text format.
fn render_text(doc: &Document) -> String {
    let mut out = String::new();
    for k in &doc.keys {
        out.push_str(&format!("[key {}]\n", k.path));
        for v in &k.values {
            // Reconstruct bytes to produce an exact, explicit literal token.
            if let Ok((_, ty, bytes)) = v.bytes() {
                out.push_str(&format!("  {} = {}\n", v.name, literal::to_token(ty, &bytes)));
            }
        }
        out.push('\n');
    }
    out
}
