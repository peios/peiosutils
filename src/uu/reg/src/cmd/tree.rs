// `reg tree <key>` — recursively list the subkey tree.

use crate::addr::{display_value_name, KeyPath};
use crate::cmd;
use crate::error::{Error, Result};
use crate::literal;
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{KeyAccess, OpenFlags};
use serde_json::json;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let depth = m.get_one::<u32>("depth").copied().unwrap_or(u32::MAX);
    let with_values = m.get_flag("values");
    let root = path.display(set.sep);

    let mut paths = Vec::new();
    if !set.json {
        println!("{root}");
    }
    walk(&path, &set, depth, with_values, "", &mut paths)?;
    if set.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "root": root, "keys": paths }))
                .unwrap_or_default()
        );
    }
    Ok(())
}

fn walk(
    path: &KeyPath,
    set: &Settings,
    depth: u32,
    with_values: bool,
    indent: &str,
    paths: &mut Vec<String>,
) -> Result<()> {
    if depth == 0 {
        return Ok(());
    }
    let target = path.display(set.sep);
    let access = if with_values {
        KeyAccess::READ
    } else {
        KeyAccess::ENUMERATE_SUB_KEYS | KeyAccess::READ_CONTROL
    };
    let key = cmd::open(path, access, OpenFlags::empty(), set)?;

    if with_values && !set.json {
        if let Ok(records) = key.query_values_batch(None) {
            for r in &records {
                println!(
                    "{indent}  {} = {} {}",
                    display_value_name(&r.name),
                    literal::type_name(r.ty),
                    literal::format_human(r.ty, &r.data),
                );
            }
        }
    }

    let children: Vec<String> = key
        .subkeys(None)
        .map(|sk| sk.map(|s| String::from_utf8_lossy(&s.name).into_owned()))
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| Error::from_peios("enumerate subkeys", &target, e))?;

    let n = children.len();
    for (i, name) in children.iter().enumerate() {
        let last = i + 1 == n;
        let branch = if last { "└── " } else { "├── " };
        let child = path.child(name);
        paths.push(child.display(set.sep));
        if !set.json {
            println!("{indent}{branch}{name}");
        }
        let next_indent = format!("{indent}{}", if last { "    " } else { "│   " });
        walk(&child, set, depth - 1, with_values, &next_indent, paths)?;
    }
    Ok(())
}
