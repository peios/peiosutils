// `reg set <key> <value> <data>` — create or update a value.

use crate::addr::{display_value_name, ValueTarget};
use crate::cmd;
use crate::error::{Error, Result};
use crate::literal;
use crate::render::{CmdOutput, Lines};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{CreateFlags, Key, KeyAccess};
use serde_json::json;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = path.display(set.sep);
    let name = match cmd::value_target(m) {
        ValueTarget::Value(n) => n,
        ValueTarget::Key => return Err(Error::Usage("set requires a value name (or @)".into())),
    };
    let data_token = m
        .get_one::<String>(crate::cli::opt::DATA)
        .ok_or_else(|| Error::Usage("missing value data".into()))?;
    let (ty, bytes) = literal::parse(data_token)?;
    let expected_seq = m.get_one::<u64>("expected-seq").copied();

    // Open (or, with -p, open/create) the key with set-value access.
    let key = if m.get_flag("parents") {
        Key::create(
            None,
            &path.to_abi(),
            KeyAccess::WRITE | KeyAccess::SET_VALUE,
            CreateFlags::empty(),
            set.layer_arg(),
            None,
        )
        .map_err(|e| Error::from_peios("create key", &target, e))?
        .0
    } else {
        cmd::open(&path, KeyAccess::SET_VALUE, Default::default(), &set)?
    };

    let mut sv = key.set_value(&name, ty, &bytes);
    if let Some(l) = set.layer_arg() {
        sv.layer(l);
    }
    if let Some(seq) = expected_seq {
        sv.expect_seq(seq);
    }
    sv.call()
        .map_err(|e| Error::from_peios("set value", &target, e))?;

    // Echo the resolved type so a broad-inference coercion is never silent
    // (spec O5). Suppressed by --quiet only for the non-surprising path.
    let mut lines = Lines::new();
    let suppress = set.quiet && !literal::is_surprising_coercion(data_token, ty);
    if !suppress {
        lines.plain(format!(
            "set {} {} = {} {}   (layer: {})",
            target,
            display_value_name(&name),
            literal::type_name(ty),
            literal::format_human(ty, &bytes),
            set.layer_label(),
        ));
    }
    let json = json!({
        "set": target,
        "name": display_value_name(&name),
        "type": literal::type_keyword(ty),
        "value": literal::format_json(ty, &bytes)["data"],
        "layer": set.layer_label(),
    });
    cmd::emit(&CmdOutput { human: lines, json }, &set);
    Ok(())
}
