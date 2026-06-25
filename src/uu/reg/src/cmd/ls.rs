// `reg ls <key>` — one-level listing of subkeys and values.

use crate::addr::display_value_name;
use crate::cmd;
use crate::error::{Error, Result};
use crate::literal;
use crate::render::{CmdOutput, Lines};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{Key, KeyAccess, OpenFlags};
use serde_json::json;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let long = m.get_flag("long");
    let keys_only = m.get_flag("keys-only");
    let values_only = m.get_flag("values-only");
    let target = path.display(set.sep);

    let key = cmd::open(&path, KeyAccess::READ, OpenFlags::empty(), &set)?;
    let out = render(&key, long, keys_only, values_only, &target)?;
    cmd::emit(&out, &set);
    Ok(())
}

fn render(
    key: &Key,
    long: bool,
    keys_only: bool,
    values_only: bool,
    target: &str,
) -> Result<CmdOutput> {
    let mut lines = Lines::new();
    let mut subkeys_json = Vec::new();
    let mut values_json = Vec::new();

    if !values_only {
        for sk in key.subkeys(None) {
            let sk = sk.map_err(|e| Error::from_peios("enumerate subkeys", target, e))?;
            let name = String::from_utf8_lossy(&sk.name).into_owned();
            if long {
                lines.plain(format!(
                    "{name}/   ({} subkeys, {} values)",
                    sk.subkey_count, sk.value_count
                ));
            } else {
                lines.plain(format!("{name}/"));
            }
            subkeys_json.push(json!({
                "name": name,
                "subkey_count": sk.subkey_count,
                "value_count": sk.value_count,
                "last_write_time": sk.last_write_time,
            }));
        }
    }

    if !keys_only {
        let mut records = key
            .query_values_batch(None)
            .map_err(|e| Error::from_peios("read values", target, e))?;
        records.sort_by(|a, b| {
            let ka = (!a.name.is_empty(), a.name.to_ascii_lowercase());
            let kb = (!b.name.is_empty(), b.name.to_ascii_lowercase());
            ka.cmp(&kb)
        });
        for r in &records {
            if long {
                lines.plain(format!(
                    "{} = {} {}   ({} bytes)",
                    display_value_name(&r.name),
                    literal::type_name(r.ty),
                    literal::format_human(r.ty, &r.data),
                    r.data.len(),
                ));
            } else {
                lines.plain(format!(
                    "{} = {}",
                    display_value_name(&r.name),
                    literal::format_human(r.ty, &r.data),
                ));
            }
            let mut j = literal::format_json(r.ty, &r.data);
            if let Some(o) = j.as_object_mut() {
                o.insert("name".into(), json!(display_value_name(&r.name)));
            }
            values_json.push(j);
        }
    }

    Ok(CmdOutput {
        human: lines,
        json: json!({ "subkeys": subkeys_json, "values": values_json }),
    })
}
