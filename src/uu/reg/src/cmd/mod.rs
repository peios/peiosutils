// Subcommand dispatch + helpers shared across commands.

use crate::addr::{KeyPath, ValueTarget};
use crate::error::{Error, Result};
use crate::render::CmdOutput;
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{Key, KeyAccess, OpenFlags};

pub mod backup;
pub mod del;
pub mod get;
pub mod hide;
pub mod info;
pub mod layer;
pub mod link;
pub mod ls;
pub mod mask;
pub mod nu; // `new` (reserved word avoided)
pub mod restore;
pub mod sd;
pub mod set;
pub mod tree;
pub mod watch;

/// Batch (apply/export) lives in its own module tree.
pub mod batch;

pub fn dispatch(matches: &ArgMatches) -> Result<()> {
    let (name, m) = matches
        .subcommand()
        .ok_or_else(|| Error::Usage("a subcommand is required".into()))?;
    match name {
        "get" => get::run(m),
        "ls" => ls::run(m),
        "tree" => tree::run(m),
        "info" => info::run(m),
        "set" => set::run(m),
        "new" => nu::run(m),
        "del" => del::run(m),
        "mask" => mask::run(m, /* set */ true),
        "unmask" => mask::run(m, /* set */ false),
        "hide" => hide::run(m, /* hide */ true),
        "unhide" => hide::run(m, /* hide */ false),
        "layer" => layer::run(m),
        "sd" => sd::run(m),
        "link" => link::run(m),
        "apply" => batch::apply::run(m),
        "export" => batch::export::run(m),
        "backup" => backup::run(m),
        "restore" => restore::run(m),
        "watch" => watch::run(m),
        other => Err(Error::Usage(format!("unknown subcommand: {other}"))),
    }
}

// --- shared helpers --------------------------------------------------------

/// Parse the required `key` positional into a [`KeyPath`].
pub fn key_path(m: &ArgMatches) -> Result<KeyPath> {
    let raw = m
        .get_one::<String>(crate::cli::opt::KEY)
        .ok_or_else(|| Error::Usage("missing key path".into()))?;
    KeyPath::parse(raw)
}

/// Parse the optional `value` positional into a [`ValueTarget`].
pub fn value_target(m: &ArgMatches) -> ValueTarget {
    ValueTarget::from_arg(m.get_one::<String>(crate::cli::opt::VALUE).map(String::as_str))
}

/// Open an existing key, mapping failures through the reg error funnel.
pub fn open(path: &KeyPath, access: KeyAccess, flags: OpenFlags, set: &Settings) -> Result<Key> {
    Key::open(None, &path.to_abi(), access, flags)
        .map_err(|e| Error::from_peios("open key", &path.display(set.sep), e))
}

/// Print a command's output in the active mode.
pub fn emit(out: &CmdOutput, set: &Settings) {
    out.print(set.json);
}

/// Report a simple mutation result: JSON when `--json`, a one-line human
/// message otherwise (suppressed by `--quiet`).
pub fn report(set: &Settings, json: serde_json::Value, human: &str) {
    if set.json {
        println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
    } else if !set.quiet {
        println!("{human}");
    }
}
