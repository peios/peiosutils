// Clap CLI surface for `token`.
//
// Subcommands are organized to match `peios/token-design.md`. Each
// subcommand inherits the target-selection flags (--self/--real/--pid/
// --tid/--peer) and the --json output flag where applicable.

use clap::{Arg, ArgAction, ArgGroup, Command, ValueHint};

/// Build the full `token` clap Command.
pub fn build() -> Command {
    Command::new("token")
        .version(uucore::crate_version!())
        .about("Inspect and manipulate Peios KACS tokens")
        .long_about(
            "Direct, debug-level access to the KACS token API. \
             See peios/token-design.md for the full surface.",
        )
        .arg_required_else_help(false)
        .subcommand_required(false)
        // Phase 2: read-only inspection.
        .subcommand(show_subcommand())
        .subcommand(query_subcommand())
        .subcommand(accessor_subcommand("user", "User SID"))
        .subcommand(accessor_subcommand("owner", "Owner SID"))
        .subcommand(accessor_subcommand("group", "Primary group SID"))
        .subcommand(accessor_subcommand("privs", "Privileges"))
        .subcommand(accessor_subcommand("groups", "Groups"))
        .subcommand(accessor_subcommand("claims", "User and device claims"))
        .subcommand(accessor_subcommand("caps", "Capabilities"))
        .subcommand(accessor_subcommand("integrity", "Integrity level"))
        .subcommand(accessor_subcommand("stats", "Token statistics"))
        .subcommand(accessor_subcommand("source", "Token source"))
        .subcommand(accessor_subcommand("origin", "Token origin"))
        .subcommand(accessor_subcommand("logon", "Logon type and SID"))
        .subcommand(accessor_subcommand("default-dacl", "Default DACL"))
        // Phase 3: mutation surface.
        .subcommand(adjust_subcommand())
        .subcommand(duplicate_subcommand())
        .subcommand(restrict_subcommand())
        .subcommand(link_subcommand())
        .subcommand(
            Command::new("linked")
                .about("Show this token's elevation-linked counterpart")
                .args(target_args())
                .arg(json_arg()),
        )
        .subcommand(impersonate_subcommand())
        .subcommand(
            Command::new("revert")
                .about("Drop any active impersonation on the calling thread")
                .arg(json_arg()),
        )
        .subcommand(create_subcommand())
        .subcommand(install_subcommand())
        // When no subcommand is given, behave as `token show --short`.
        .args(target_args())
        .args(sid_style_args())
        .arg(json_arg())
        .arg(
            Arg::new("short")
                .long("short")
                .help("One-line summary (default when no subcommand is given)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("all")
                .long("all")
                .help("Dump every query class")
                .action(ArgAction::SetTrue),
        )
}

fn show_subcommand() -> Command {
    Command::new("show")
        .about("Show token contents (default subcommand)")
        .args(target_args())
        .args(sid_style_args())
        .arg(json_arg())
        .arg(
            Arg::new("short")
                .long("short")
                .help("One-line summary")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("all")
                .long("all")
                .help("Dump every query class")
                .action(ArgAction::SetTrue),
        )
}

fn query_subcommand() -> Command {
    Command::new("query")
        .about("Raw KACS query of a single token-info class (JSON output)")
        .arg(
            Arg::new("class")
                .required(true)
                .help("Token-info class name (e.g. user, groups, privileges, ...)")
                .value_hint(ValueHint::Other),
        )
        .args(target_args())
        .arg(json_arg())
}

fn accessor_subcommand(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .args(target_args())
        .args(sid_style_args())
        .arg(json_arg())
}

// ---------------------------------------------------------------------------
// Phase 3 subcommand builders.
// ---------------------------------------------------------------------------

fn adjust_subcommand() -> Command {
    Command::new("adjust")
        .about("Mutate token state (privileges, groups, default DACL, session)")
        .subcommand_required(true)
        .subcommand(
            Command::new("privs")
                .about("Adjust privileges: <name|luid>=<enabled|disabled|removed> ...")
                .arg(
                    Arg::new("entries")
                        .num_args(1..)
                        .required(true)
                        .help("One or more <name|luid>=<state> entries"),
                )
                .args(target_args())
                .arg(json_arg()),
        )
        .subcommand(
            Command::new("groups")
                .about("Adjust groups by index: <idx>=<enabled|disabled> ...")
                .arg(
                    Arg::new("entries")
                        .num_args(1..)
                        .required(true)
                        .help("One or more <idx>=<state> entries"),
                )
                .args(target_args())
                .arg(json_arg()),
        )
        .subcommand(
            Command::new("default")
                .about("Adjust default DACL and owner/group indices")
                .arg(
                    Arg::new("dacl")
                        .long("dacl")
                        .value_name("SDDL")
                        .help("Replace default DACL with the one in this SDDL string"),
                )
                .arg(
                    Arg::new("owner-idx")
                        .long("owner-idx")
                        .value_name("N")
                        .help("Owner index into the token's group list")
                        .value_parser(clap::value_parser!(u16)),
                )
                .arg(
                    Arg::new("group-idx")
                        .long("group-idx")
                        .value_name("N")
                        .help("Primary-group index into the token's group list")
                        .value_parser(clap::value_parser!(u16)),
                )
                .args(target_args())
                .arg(json_arg()),
        )
        .subcommand(
            Command::new("session")
                .about("Replace the token's session id")
                .arg(
                    Arg::new("session-id")
                        .required(true)
                        .help("New session id (u32)")
                        .value_parser(clap::value_parser!(u32)),
                )
                .args(target_args())
                .arg(json_arg()),
        )
}

fn duplicate_subcommand() -> Command {
    Command::new("duplicate")
        .alias("dup")
        .about("Duplicate a token (changes type / impersonation level / access mask)")
        .arg(
            Arg::new("type")
                .long("type")
                .value_name("TYPE")
                .help("primary | impersonation")
                .value_parser(["primary", "impersonation", "imp"]),
        )
        .arg(
            Arg::new("level")
                .long("level")
                .value_name("LEVEL")
                .help("anonymous | identification | impersonation | delegation")
                .value_parser([
                    "anonymous",
                    "anon",
                    "identification",
                    "id",
                    "impersonation",
                    "imp",
                    "delegation",
                    "del",
                ]),
        )
        .arg(
            Arg::new("access")
                .long("access")
                .value_name("MASK")
                .help("Access mask (hex or decimal)")
                .value_parser(parse_u32_mask),
        )
        .args(target_args())
        .arg(json_arg())
}

fn restrict_subcommand() -> Command {
    Command::new("restrict")
        .about("Produce a restricted variant of a token")
        .arg(
            Arg::new("drop-privs")
                .long("drop-privs")
                .value_name("MASK|NAMES")
                .help("Bitmask or comma-separated privilege names to drop"),
        )
        .arg(
            Arg::new("deny")
                .long("deny")
                .value_name("IDX,IDX,...")
                .help("Group-list indices to mark deny-only"),
        )
        .arg(
            Arg::new("restrict")
                .long("restrict")
                .value_name("SID,SID,...")
                .help("Restricted SIDs (raw or SDDL alias)"),
        )
        .arg(
            Arg::new("flags")
                .long("flags")
                .value_name("MASK")
                .help("KACS_RESTRICT_* flag bits")
                .value_parser(parse_u32_mask),
        )
        .args(target_args())
        .arg(json_arg())
}

fn link_subcommand() -> Command {
    Command::new("link")
        .about("Link two tokens as an elevation pair (UAC-style)")
        .arg(
            Arg::new("elevated")
                .long("elevated")
                .value_name("FD")
                .required(true)
                .value_parser(clap::value_parser!(i32)),
        )
        .arg(
            Arg::new("filtered")
                .long("filtered")
                .value_name("FD")
                .required(true)
                .value_parser(clap::value_parser!(i32)),
        )
        .arg(
            Arg::new("session")
                .long("session")
                .value_name("ID")
                .required(true)
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(json_arg())
}

fn impersonate_subcommand() -> Command {
    Command::new("impersonate")
        .about("Start impersonating a token (use -- <cmd> to exec under impersonation)")
        .args(target_args())
        .arg(
            Arg::new("exec")
                .num_args(0..)
                .trailing_var_arg(true)
                .allow_hyphen_values(true)
                .last(true)
                .help("Command (and args) to execute under the impersonating token"),
        )
        .arg(json_arg())
}

fn create_subcommand() -> Command {
    Command::new("create")
        .about("Create a token from a kernel-format spec (binary)")
        .long_about(
            "Reads the spec bytes from <SPEC> (a file path, or `-` for stdin) and \
             passes them to kacs_create_token. The wire format is the fixed 192-byte \
             token-spec header (version 2) plus referenced sections; see the kernel's \
             create_from_spec parser. JSON sugar is not yet wired — pre-build the \
             spec or use kunit-style helpers.",
        )
        .arg(
            Arg::new("spec")
                .value_name("SPEC")
                .required(true)
                .help("Path to spec bytes (or `-` for stdin)"),
        )
        .arg(json_arg())
}

fn install_subcommand() -> Command {
    Command::new("install")
        .about("Create + install a token as the caller's primary token")
        .arg(
            Arg::new("spec")
                .value_name("SPEC")
                .required(true)
                .help("Path to spec bytes (or `-` for stdin)"),
        )
        .arg(json_arg())
}

/// Parse `0x...` or decimal into `u32`. Used as a clap value parser.
fn parse_u32_mask(s: &str) -> Result<u32, String> {
    let trimmed = s.trim();
    if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).map_err(|e| format!("{e}"))
    } else {
        trimmed.parse::<u32>().map_err(|e| format!("{e}"))
    }
}

fn target_args() -> Vec<Arg> {
    vec![
        Arg::new("self")
            .long("self")
            .help("Caller's own token (default)")
            .action(ArgAction::SetTrue),
        Arg::new("real")
            .long("real")
            .help("Caller's real (primary) token, not the effective one")
            .action(ArgAction::SetTrue),
        Arg::new("pid")
            .long("pid")
            .value_name("PID")
            .help("Open the token of process PID")
            .value_parser(clap::value_parser!(i32)),
        Arg::new("tid")
            .long("tid")
            .value_name("TID")
            .help("Open the impersonation token of thread TID (requires --pid)")
            .value_parser(clap::value_parser!(i32)),
        Arg::new("peer")
            .long("peer")
            .value_name("SOCK_FD")
            .help("Open the AF_UNIX peer-captured token from socket fd")
            .value_parser(clap::value_parser!(i32)),
    ]
}

fn sid_style_args() -> Vec<Arg> {
    vec![
        Arg::new("raw")
            .long("raw")
            .help("Render SIDs in raw form only (S-1-...)")
            .action(ArgAction::SetTrue),
        Arg::new("label")
            .long("label")
            .help("Render SIDs as labels where known; fall back to raw")
            .action(ArgAction::SetTrue),
    ]
}

fn json_arg() -> Arg {
    Arg::new("json")
        .long("json")
        .help("Emit JSON instead of human-readable output")
        .action(ArgAction::SetTrue)
}

/// Apply the `target-args`-style mutual-exclusion group to a subcommand.
/// Kept as a helper so the same group definition lives in one place.
#[allow(dead_code)]
pub fn target_group() -> ArgGroup {
    ArgGroup::new("target")
        .args(["self", "real", "pid", "peer"])
        .required(false)
        .multiple(false)
}
