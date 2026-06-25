// `reg get <key> [value]` — read the effective value, or list a key's values.

use crate::addr::{display_value_name, ValueTarget};
use crate::cmd;
use crate::error::{Error, Result};
use crate::literal;
use crate::render::{CmdOutput, Lines};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{Key, KeyAccess, OpenFlags, RegValue};
use serde_json::json;
use std::io::Write;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = cmd::value_target(m);
    let with_layers = m.get_flag("layers");
    let raw = m.get_flag("raw");

    let mut flags = OpenFlags::empty();
    if m.get_flag("no-follow") {
        flags |= OpenFlags::OPEN_LINK;
    }
    let key = cmd::open(&path, KeyAccess::READ, flags, &set)?;

    match target {
        ValueTarget::Value(name) => {
            let v = key
                .query_value(&name, None)
                .map_err(|e| Error::from_peios("query value", &path.display(set.sep), e))?;
            if raw {
                std::io::stdout().write_all(&v.data).ok();
                return Ok(());
            }
            let out = render_one(&name, &v, with_layers);
            cmd::emit(&out, &set);
            Ok(())
        }
        ValueTarget::Key => {
            let out = render_list(&key, with_layers, &path.display(set.sep))?;
            cmd::emit(&out, &set);
            Ok(())
        }
    }
}

/// A single value (`get <key> <value>`): bare value by default; with `-L`, the
/// winning layer + sequence (full shadowed stack pending an ABI — spec O7).
fn render_one(name: &[u8], v: &RegValue, with_layers: bool) -> CmdOutput {
    let mut lines = Lines::new();
    if with_layers {
        let layer = String::from_utf8_lossy(&v.layer);
        let layer = if layer.is_empty() { "base".into() } else { layer };
        lines.plain(format!(
            "{} = {} {}   (layer: {}, seq {})",
            display_value_name(name),
            literal::type_name(v.ty),
            literal::format_human(v.ty, &v.data),
            layer,
            v.sequence,
        ));
    } else {
        lines.plain(literal::format_bare(v.ty, &v.data));
    }
    let mut json = literal::format_json(v.ty, &v.data);
    if let Some(o) = json.as_object_mut() {
        o.insert("name".into(), json!(display_value_name(name)));
        o.insert("sequence".into(), json!(v.sequence));
        o.insert(
            "layer".into(),
            json!(String::from_utf8_lossy(&v.layer).into_owned()),
        );
    }
    CmdOutput { human: lines, json }
}

/// List a key's effective values (default value first), via the batch read.
fn render_list(key: &Key, _with_layers: bool, target: &str) -> Result<CmdOutput> {
    let mut records = key
        .query_values_batch(None)
        .map_err(|e| Error::from_peios("read values", target, e))?;
    // Default (empty-name) value first, then case-insensitive by name.
    records.sort_by(|a, b| {
        let ka = (!a.name.is_empty(), a.name.to_ascii_lowercase());
        let kb = (!b.name.is_empty(), b.name.to_ascii_lowercase());
        ka.cmp(&kb)
    });

    let mut lines = Lines::new();
    let mut arr = Vec::new();
    for r in &records {
        lines.plain(format!(
            "{} = {} {}",
            display_value_name(&r.name),
            literal::type_name(r.ty),
            literal::format_human(r.ty, &r.data),
        ));
        let mut j = literal::format_json(r.ty, &r.data);
        if let Some(o) = j.as_object_mut() {
            o.insert("name".into(), json!(display_value_name(&r.name)));
        }
        arr.push(j);
    }
    Ok(CmdOutput {
        human: lines,
        json: json!({ "values": arr }),
    })
}
