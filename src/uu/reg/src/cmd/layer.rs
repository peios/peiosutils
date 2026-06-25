// `reg layer …` — manage layers.
//
// Layers are configured through their metadata keys under
// `Machine\System\Registry\Layers\<name>\` (PSD-005 §2.6): a `Precedence`
// (DWORD), an `Enabled` (DWORD 0/1), and an informational `Owner` (SZ SID).

use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{CreateFlags, Key, KeyAccess, OpenFlags, ValueType};
use serde_json::json;

const LAYERS_BASE: &str = r"Machine\System\Registry\Layers";

pub fn run(m: &ArgMatches) -> Result<()> {
    let (name, sub) = m
        .subcommand()
        .ok_or_else(|| Error::Usage("layer: subcommand required (ls|new|set|del)".into()))?;
    match name {
        "ls" => ls(sub),
        "new" => new(sub),
        "set" => set(sub),
        "del" => del(sub),
        other => Err(Error::Usage(format!("unknown layer subcommand: {other}"))),
    }
}

fn layers_key(access: KeyAccess) -> Result<Key> {
    Key::open(None, LAYERS_BASE, access, OpenFlags::empty())
        .map_err(|e| Error::from_peios("open layers key", LAYERS_BASE, e))
}

fn dword(key: &Key, name: &[u8]) -> Option<u32> {
    let v = key.query_value(name, None).ok()?;
    (v.data.len() == 4).then(|| u32::from_le_bytes(v.data[..4].try_into().unwrap()))
}

fn sz(key: &Key, name: &[u8]) -> Option<String> {
    let v = key.query_value(name, None).ok()?;
    let end = v.data.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
    Some(String::from_utf8_lossy(&v.data[..end]).into_owned())
}

fn ls(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let base = layers_key(KeyAccess::READ)?;
    let mut rows = Vec::new();
    for sk in base.subkeys(None) {
        let sk = sk.map_err(|e| Error::from_peios("enumerate layers", LAYERS_BASE, e))?;
        let name = String::from_utf8_lossy(&sk.name).into_owned();
        let lk = Key::open(
            Some(&base),
            &name,
            KeyAccess::READ,
            OpenFlags::empty(),
        )
        .map_err(|e| Error::from_peios("open layer", &name, e))?;
        let precedence = dword(&lk, b"Precedence").unwrap_or(0);
        let enabled = dword(&lk, b"Enabled").map(|v| v != 0).unwrap_or(true);
        let owner = sz(&lk, b"Owner");
        rows.push((name, precedence, enabled, owner));
    }
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    if set.json {
        let arr: Vec<_> = rows
            .iter()
            .map(|(n, p, e, o)| json!({ "name": n, "precedence": p, "enabled": e, "owner": o }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&json!({ "layers": arr })).unwrap_or_default());
    } else {
        for (n, p, e, o) in &rows {
            println!(
                "{n:<24} prec={p:<6} {}{}",
                if *e { "enabled " } else { "disabled" },
                o.as_deref().map(|s| format!("  owner={s}")).unwrap_or_default()
            );
        }
    }
    Ok(())
}

fn new(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let name = m.get_one::<String>("name").unwrap();
    let precedence = m.get_one::<u32>("precedence").copied().unwrap_or(0);
    let enabled = !m.get_flag("disabled");
    let owner = m.get_one::<String>("owner");

    let base = layers_key(KeyAccess::CREATE_SUB_KEY | KeyAccess::READ)?;
    let (lk, _) = Key::create(
        Some(&base),
        name,
        KeyAccess::WRITE | KeyAccess::SET_VALUE,
        CreateFlags::empty(),
        None,
        None,
    )
    .map_err(|e| Error::from_peios("create layer", name, e))?;

    write_dword(&lk, b"Precedence", precedence, name)?;
    write_dword(&lk, b"Enabled", enabled as u32, name)?;
    if let Some(o) = owner {
        write_sz(&lk, b"Owner", o, name)?;
    }
    cmd::report(
        &set,
        json!({ "layer": name, "precedence": precedence, "enabled": enabled }),
        &format!("created layer {name} (precedence {precedence}, {})",
                 if enabled { "enabled" } else { "disabled" }),
    );
    Ok(())
}

fn set(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let name = m.get_one::<String>("name").unwrap();
    let base = layers_key(KeyAccess::READ)?;
    let lk = Key::open(Some(&base), name, KeyAccess::SET_VALUE, OpenFlags::empty())
        .map_err(|e| Error::from_peios("open layer", name, e))?;

    if let Some(p) = m.get_one::<u32>("precedence").copied() {
        write_dword(&lk, b"Precedence", p, name)?;
    }
    if m.get_flag("enable") {
        write_dword(&lk, b"Enabled", 1, name)?;
    }
    if m.get_flag("disable") {
        write_dword(&lk, b"Enabled", 0, name)?;
    }
    if let Some(o) = m.get_one::<String>("owner") {
        write_sz(&lk, b"Owner", o, name)?;
    }
    cmd::report(&set, json!({ "layer": name, "updated": true }), &format!("updated layer {name}"));
    Ok(())
}

fn del(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let name = m.get_one::<String>("name").unwrap();
    if !set.confirm(&format!("Delete layer {name} and all its entries?"))? {
        return Err(Error::Usage("aborted".into()));
    }
    let base = layers_key(KeyAccess::READ)?;
    let lk = Key::open(
        Some(&base),
        name,
        KeyAccess::DELETE | KeyAccess::SET_VALUE,
        OpenFlags::empty(),
    )
    .map_err(|e| Error::from_peios("open layer", name, e))?;
    // Best-effort: clear the known metadata values, then delete the key.
    for v in [&b"Precedence"[..], b"Enabled", b"Owner"] {
        lk.delete_value(v, None, None).ok();
    }
    lk.delete_key(None, None)
        .map_err(|e| Error::from_peios("delete layer", name, e))?;
    cmd::report(&set, json!({ "layer": name, "deleted": true }), &format!("deleted layer {name}"));
    Ok(())
}

fn write_dword(key: &Key, name: &[u8], v: u32, layer: &str) -> Result<()> {
    key.set_value(name, ValueType::DWORD, &v.to_le_bytes())
        .call()
        .map_err(|e| Error::from_peios("write layer metadata", layer, e))
}

fn write_sz(key: &Key, name: &[u8], v: &str, layer: &str) -> Result<()> {
    let mut bytes = v.as_bytes().to_vec();
    bytes.push(0);
    key.set_value(name, ValueType::SZ, &bytes)
        .call()
        .map_err(|e| Error::from_peios("write layer metadata", layer, e))
}
