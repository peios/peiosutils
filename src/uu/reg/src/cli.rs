// Clap CLI surface for `reg`. Subcommands match docs/reg-spec.md §4.

use clap::{Arg, ArgAction, Command};

/// Stable argument ids.
pub mod opt {
    pub const KEY: &str = "key";
    pub const VALUE: &str = "value";
    pub const DATA: &str = "data";
    pub const LAYER: &str = "layer";
    pub const JSON: &str = "json";
    pub const VERBOSE: &str = "verbose";
    pub const QUIET: &str = "quiet";
    pub const SEP: &str = "sep";
    pub const YES: &str = "yes";
}

pub fn build() -> Command {
    Command::new("reg")
        .version(uucore::crate_version!())
        .about("Query and manipulate the Peios registry (LCS)")
        .long_about(
            "Inspect and modify the live layered configuration registry. \
             See docs/reg-spec.md for the full surface.",
        )
        .arg_required_else_help(true)
        .subcommand_required(true)
        .subcommand(
            common(Command::new("get").about("Read the effective value (or list a key's values)"))
                .arg(key_arg())
                .arg(value_arg())
                .arg(flag("layers", 'L', "Annotate with the winning layer + sequence"))
                .arg(flag("raw", '\0', "Write raw value bytes to stdout"))
                .arg(flag("no-follow", '\0', "Operate on a symlink key, not its target")),
        )
        .subcommand(
            common(Command::new("ls").about("List a key's subkeys and values"))
                .arg(key_arg())
                .arg(flag("long", 'l', "Long form: types, sizes, timestamps"))
                .arg(flag("keys-only", '\0', "List only subkeys"))
                .arg(flag("values-only", '\0', "List only values")),
        )
        .subcommand(
            common(Command::new("tree").about("Recursively list the subkey tree"))
                .arg(key_arg())
                .arg(
                    Arg::new("depth")
                        .long("depth")
                        .value_name("N")
                        .value_parser(clap::value_parser!(u32))
                        .help("Limit recursion depth"),
                )
                .arg(flag("values", '\0', "Include values at each node")),
        )
        .subcommand(
            common(Command::new("info").about("Show key metadata"))
                .arg(key_arg())
                .arg(flag("no-follow", '\0', "Inspect a symlink key, not its target")),
        )
        .subcommand(
            common(Command::new("set").about("Create or update a value"))
                .arg(key_arg())
                .arg(value_arg().required(true))
                .arg(Arg::new(opt::DATA).required(true).help("Value data (type: prefix or inferred)"))
                .arg(layer_arg())
                .arg(flag("parents", 'p', "Create missing ancestor keys"))
                .arg(
                    Arg::new("expected-seq")
                        .long("expected-seq")
                        .value_name("N")
                        .value_parser(clap::value_parser!(u64))
                        .help("CAS guard: apply only if the current sequence matches"),
                ),
        )
        .subcommand(
            common(Command::new("new").about("Create a key"))
                .arg(key_arg())
                .arg(layer_arg())
                .arg(flag("parents", 'p', "Create missing ancestor keys"))
                .arg(flag("volatile", '\0', "Create a volatile (RAM-only) key")),
        )
        .subcommand(
            common(Command::new("del").about("Delete a value or a key"))
                .visible_alias("delete")
                .arg(key_arg())
                .arg(value_arg())
                .arg(layer_arg())
                .arg(flag("recursive", 'r', "Recursively delete a key and its children"))
                .arg(yes_arg()),
        )
        .subcommand(
            common(Command::new("mask").about("Tombstone a value (or all values) in a layer"))
                .arg(key_arg())
                .arg(value_arg())
                .arg(layer_arg())
                .arg(flag("all", '\0', "Blanket tombstone: mask all lower values")),
        )
        .subcommand(
            common(Command::new("unmask").about("Clear a tombstone / blanket tombstone"))
                .arg(key_arg())
                .arg(value_arg())
                .arg(layer_arg())
                .arg(flag("all", '\0', "Clear the blanket tombstone")),
        )
        .subcommand(
            common(Command::new("hide").about("Hide a key in a layer"))
                .arg(key_arg())
                .arg(layer_arg()),
        )
        .subcommand(
            common(Command::new("unhide").about("Clear a key hide in a layer"))
                .arg(key_arg())
                .arg(layer_arg()),
        )
        .subcommand(layer_subcommand())
        .subcommand(
            common(Command::new("sd").about("Show or set a key's security descriptor (SDDL)"))
                .arg(key_arg())
                .arg(Arg::new("set").long("set").value_name("SDDL").help("Apply this SDDL"))
                .arg(flag("owner", '\0', "Scope to the owner"))
                .arg(flag("group", '\0', "Scope to the group"))
                .arg(flag("dacl", '\0', "Scope to the DACL"))
                .arg(flag("sacl", '\0', "Scope to the SACL")),
        )
        .subcommand(
            common(Command::new("link").about("Create a symlink key"))
                .arg(key_arg())
                .arg(Arg::new("target").required(true).help("Absolute target key path"))
                .arg(layer_arg()),
        )
        .subcommand(
            common(Command::new("apply").about("Apply a batch of operations atomically"))
                .arg(Arg::new("file").required(true).help("Batch file (text or JSON); - for stdin"))
                .arg(yes_arg()),
        )
        .subcommand(
            common(Command::new("export").about("Dump a subtree to a batch file"))
                .arg(key_arg())
                .arg(Arg::new("file").help("Output file; - or omitted for stdout"))
                .arg(layer_arg()),
        )
        .subcommand(
            common(Command::new("backup").about("Binary snapshot of a key + subtree"))
                .arg(key_arg())
                .arg(Arg::new("file").required(true).help("Output snapshot file")),
        )
        .subcommand(
            common(Command::new("restore").about("Replace a key + subtree from a snapshot"))
                .arg(key_arg())
                .arg(Arg::new("file").required(true).help("Input snapshot file"))
                .arg(yes_arg()),
        )
        .subcommand(
            common(Command::new("watch").about("Stream change notifications"))
                .arg(key_arg())
                .arg(flag("subtree", '\0', "Watch descendants too"))
                .arg(
                    Arg::new("filter")
                        .long("filter")
                        .value_name("LIST")
                        .help("Comma list of: value,subkey,sd (default: all)"),
                )
                .arg(
                    Arg::new("count")
                        .long("count")
                        .value_name("N")
                        .value_parser(clap::value_parser!(u64))
                        .help("Exit after N events"),
                ),
        )
}

fn layer_subcommand() -> Command {
    common(Command::new("layer").about("Manage layers"))
        .subcommand_required(true)
        .subcommand(common(Command::new("ls").about("List layers")).arg(flag("long", 'l', "Long form")))
        .subcommand(
            common(Command::new("new").about("Create a layer"))
                .arg(Arg::new("name").required(true).help("Layer name"))
                .arg(
                    Arg::new("precedence")
                        .long("precedence")
                        .value_name("N")
                        .value_parser(clap::value_parser!(u32))
                        .help("Precedence (higher wins; >0 needs SeTcbPrivilege)"),
                )
                .arg(Arg::new("owner").long("owner").value_name("SID").help("Owner SID (informational)"))
                .arg(flag("disabled", '\0', "Create disabled")),
        )
        .subcommand(
            common(Command::new("set").about("Modify a layer"))
                .arg(Arg::new("name").required(true).help("Layer name"))
                .arg(
                    Arg::new("precedence")
                        .long("precedence")
                        .value_name("N")
                        .value_parser(clap::value_parser!(u32)),
                )
                .arg(flag("enable", '\0', "Enable the layer"))
                .arg(flag("disable", '\0', "Disable the layer"))
                .arg(Arg::new("owner").long("owner").value_name("SID")),
        )
        .subcommand(
            common(Command::new("del").about("Delete a layer"))
                .arg(Arg::new("name").required(true).help("Layer name"))
                .arg(yes_arg()),
        )
}

// --- shared arg builders ---------------------------------------------------

/// Attach the flags every subcommand accepts (§4.0).
fn common(cmd: Command) -> Command {
    cmd.arg(flag(opt::JSON, '\0', "Structured JSON output"))
        .arg(flag(opt::VERBOSE, 'v', "Verbose output"))
        .arg(flag(opt::QUIET, 'q', "Suppress non-essential output"))
        .arg(
            Arg::new(opt::SEP)
                .long(opt::SEP)
                .value_name("CHAR")
                .help("Path display separator: \\ (default) or /"),
        )
}

fn key_arg() -> Arg {
    Arg::new(opt::KEY).required(true).help("Key path (/ or \\ separated)")
}

fn value_arg() -> Arg {
    Arg::new(opt::VALUE).help("Value name (@ = default value)")
}

fn layer_arg() -> Arg {
    Arg::new(opt::LAYER)
        .long(opt::LAYER)
        .value_name("NAME")
        .help("Target layer (default: base)")
}

fn yes_arg() -> Arg {
    flag(opt::YES, 'y', "Skip the confirmation prompt")
}

/// A boolean flag; pass `'\0'` for no short form.
fn flag(name: &'static str, short: char, help: &'static str) -> Arg {
    let mut a = Arg::new(name).long(name).help(help).action(ArgAction::SetTrue);
    if short != '\0' {
        a = a.short(short);
    }
    a
}
