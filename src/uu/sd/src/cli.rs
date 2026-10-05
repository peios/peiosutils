// clap setup. Subcommand surface mirrors `peios/sd-design.md`.

use clap::{Arg, ArgAction, Command};

pub fn build() -> Command {
    Command::new("sd")
        .version(uucore::crate_version!())
        .about("Manage Peios Security Descriptors on files")
        .subcommand_required(false)
        .arg_required_else_help(false)
        .subcommand(
            Command::new("show")
                .about("Show the SD on a path")
                .arg(path_arg())
                .arg(json_flag())
                .arg(sddl_flag())
                .arg(raw_sid_flag())
                .arg(label_sid_flag())
                .arg(all_flag())
                .arg(no_follow_flag()),
        )
        .subcommand(
            Command::new("allow")
                .about("Append an allow ACE for one or more principals")
                .arg(path_arg())
                .arg(principal_perms_arg("PRINCIPAL:PERMS"))
                .arg(flags_arg())
                .arg(if_arg())
                .arg(replace_arg())
                .arg(recursive_arg())
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("deny")
                .about("Append a deny ACE for one or more principals")
                .arg(path_arg())
                .arg(principal_perms_arg("PRINCIPAL:PERMS"))
                .arg(flags_arg())
                .arg(if_arg())
                .arg(replace_arg())
                .arg(recursive_arg())
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("remove")
                .about("Drop all DACL ACEs for one or more principals")
                .arg(path_arg())
                .arg(principals_arg())
                .arg(
                    Arg::new("allow-empty")
                        .long("allow-empty")
                        .help("Allow producing a present-but-empty DACL (denies everyone)")
                        .action(ArgAction::SetTrue),
                )
                .arg(recursive_arg())
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("audit")
                .about("Append an audit ACE to the SACL")
                .arg(path_arg())
                .arg(principal_perms_arg("PRINCIPAL:PERMS:success|failure|both"))
                .arg(flags_arg())
                .arg(if_arg())
                .arg(replace_arg())
                .arg(recursive_arg())
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("unaudit")
                .about("Drop all SACL ACEs for one or more principals")
                .arg(path_arg())
                .arg(principals_arg())
                .arg(recursive_arg())
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("owner")
                .about("Set the SD's owner SID")
                .arg(path_arg())
                .arg(Arg::new("principal").required(true).value_name("PRINCIPAL"))
                .arg(recursive_arg())
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("group")
                .about("Set the SD's group SID")
                .arg(path_arg())
                .arg(Arg::new("principal").required(true).value_name("PRINCIPAL"))
                .arg(recursive_arg())
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("integrity")
                .about("Set the mandatory integrity label")
                .arg(path_arg())
                .arg(
                    Arg::new("level").required(true).value_name("LEVEL").help(
                        "untrusted|low|medium|medium-plus|high|system|protected",
                    ),
                )
                .arg(
                    Arg::new("policy")
                        .long("policy")
                        .value_name("BITS")
                        .help("Comma-separated mandatory-label policy bits (NW,NR,NX)"),
                )
                .arg(recursive_arg())
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("inherit")
                .about("Toggle inheritance protection (SE_DACL_PROTECTED)")
                .arg(path_arg())
                .arg(
                    Arg::new("mode")
                        .required(true)
                        .value_name("on|off")
                        .help("`on` clears protection; `off` sets it"),
                )
                .arg(
                    Arg::new("strip-inherited")
                        .long("strip-inherited")
                        .help("When turning protection off, also drop existing inherited ACEs")
                        .action(ArgAction::SetTrue),
                )
                .arg(recursive_arg())
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("reset")
                .about("Drop local explicit DACL ACEs and re-inherit from parent")
                .arg(path_arg())
                .arg(recursive_arg())
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("propagate")
                .about("Push inheritance to descendants")
                .arg(path_arg())
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("set")
                .about("Replace the SD wholesale (SDDL or binary)")
                .arg(path_arg())
                .arg(
                    Arg::new("sddl")
                        .required_unless_present("binary")
                        .value_name("SDDL")
                        .help("SDDL string (or `-` for stdin)"),
                )
                .arg(
                    Arg::new("binary")
                        .long("binary")
                        .value_name("FILE")
                        .help("Read raw self-relative SD bytes from FILE (or `-` for stdin)"),
                )
                .arg(
                    Arg::new("components")
                        .long("components")
                        .value_name("LIST")
                        .help("Override the SecurityInfo bits inferred from SDDL"),
                )
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("check")
                .about("Run an access-check simulation")
                .arg(path_arg())
                .arg(
                    Arg::new("perms")
                        .required(true)
                        .value_name("PERMS")
                        .help("Desired access mask"),
                )
                .arg(
                    Arg::new("pid")
                        .long("pid")
                        .value_name("PID")
                        .value_parser(clap::value_parser!(i32))
                        .help("Check against this process's token instead of self"),
                )
                .arg(
                    Arg::new("explain")
                        .long("explain")
                        .action(ArgAction::SetTrue),
                )
                .arg(no_follow_flag())
                .arg(json_flag()),
        )
}

fn path_arg() -> Arg {
    Arg::new("path").required(true).value_name("PATH")
}

fn principal_perms_arg(name: &'static str) -> Arg {
    Arg::new("specs")
        .required(true)
        .num_args(1..)
        .value_name(name)
}

fn principals_arg() -> Arg {
    Arg::new("principals")
        .required(true)
        .num_args(1..)
        .value_name("PRINCIPAL")
}

fn flags_arg() -> Arg {
    Arg::new("flags")
        .long("flags")
        .value_name("LIST")
        .help("Comma-separated ACE flags (CI,OI,NP,IO; `none` clears)")
}

fn if_arg() -> Arg {
    Arg::new("if")
        .long("if")
        .value_name("EXPR")
        .help("Conditional ACE expression (SDDL conditional form)")
}

fn replace_arg() -> Arg {
    Arg::new("replace")
        .long("replace")
        .action(ArgAction::SetTrue)
        .help("Drop existing ACEs for this principal+kind before appending")
}

fn recursive_arg() -> Arg {
    Arg::new("recursive")
        .long("recursive")
        .short('r')
        .action(ArgAction::SetTrue)
        .help("Apply to every descendant of PATH")
}

fn no_follow_flag() -> Arg {
    Arg::new("no-follow-symlinks")
        .long("no-follow-symlinks")
        .short('P')
        .action(ArgAction::SetTrue)
        .help("Operate on the symlink itself, not its target")
}

fn json_flag() -> Arg {
    Arg::new("json")
        .long("json")
        .action(ArgAction::SetTrue)
        .help("Emit JSON instead of human-readable output")
}

fn sddl_flag() -> Arg {
    Arg::new("sddl")
        .long("sddl")
        .action(ArgAction::SetTrue)
        .help("Render the SD as SDDL")
}

fn raw_sid_flag() -> Arg {
    Arg::new("raw")
        .long("raw")
        .action(ArgAction::SetTrue)
        .help("Render SIDs as raw S-... only")
}

fn label_sid_flag() -> Arg {
    Arg::new("label")
        .long("label")
        .action(ArgAction::SetTrue)
        .help("Render SIDs as label only (fall back to raw)")
}

fn all_flag() -> Arg {
    Arg::new("all")
        .long("all")
        .action(ArgAction::SetTrue)
        .help("Verbose: dump every decoded flag and raw mask alongside")
}
