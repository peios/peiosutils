// Subcommand dispatch + helpers shared across commands.

use crate::addr::{KeyPath, ValueTarget};
use crate::error::{Error, Result};
use crate::render::CmdOutput;
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{CreateFlags, Key, KeyAccess, OpenFlags};

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

/// Create every missing ancestor of `path`, outermost first (`-p/--parents`).
///
/// `Key::create` creates exactly the key it is named, so a path with more than
/// one missing ancestor has to be walked a component at a time; doing otherwise
/// fails with "not found" on the second missing level (PEI-513).
///
/// Each ancestor is opened-or-created with `CREATE_SUB_KEY`, the only right the
/// walk actually needs: an ancestor exists here to hold the next component and
/// nothing more, so asking for `WRITE` would refuse paths whose existing
/// ancestors the caller may extend but not modify.
///
/// Ancestors are created in the same layer as the leaf but never inherit its
/// create flags. `--volatile` describes the key the user asked for, and the
/// rule in docs/reg-spec.md §4.2 runs the other way — a volatile key's children
/// must also be volatile — so a volatile leaf beneath persistent ancestors is
/// well-formed, while persistent ancestors implied by a volatile leaf would not
/// be.
pub fn create_ancestors(path: &KeyPath, layer: Option<&str>, set: &Settings) -> Result<()> {
    for ancestor in path.ancestors() {
        Key::create(
            None,
            &ancestor.to_abi(),
            KeyAccess::CREATE_SUB_KEY,
            CreateFlags::empty(),
            layer,
            None,
        )
        .map_err(|e| Error::from_peios("create key", &ancestor.display(set.sep), e))?;
    }
    Ok(())
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
