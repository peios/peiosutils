use clap::{Arg, ArgAction, Command};

pub mod opt {
    pub const JSON: &str = "json";
    pub const MOUNT: &str = "mount";
    pub const PATH: &str = "path";
}

pub fn build() -> Command {
    Command::new("stratafs")
        .version(uucore::crate_version!())
        .about("Inspect and explain StrataFS mounts")
        .long_about(
            "Read-only inspection of StrataFS mount configuration, path resolution, \
             write and removal routing, create-stratum state, and overrides.",
        )
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("list")
                .about("List StrataFS mounts and their strata")
                .arg(
                    Arg::new(opt::MOUNT)
                        .value_name("MOUNT")
                        .value_parser(clap::builder::OsStringValueParser::new()),
                )
                .arg(json_arg()),
        )
        .subcommand(
            Command::new("resolve")
                .about("Explain why a merged path resolves as it does")
                .arg(path_arg())
                .arg(json_arg()),
        )
        .subcommand(
            Command::new("origin")
                .about("Print the real provider path from system.stratafs.origin")
                .arg(path_arg()),
        )
        .subcommand(
            Command::new("sweep")
                .about("Classify entries in a create stratum as gap, override, or shadowed")
                .arg(
                    Arg::new(opt::MOUNT)
                        .value_name("MOUNT")
                        .value_parser(clap::builder::OsStringValueParser::new()),
                )
                .arg(json_arg()),
        )
        .subcommand(
            Command::new("diff")
                .about("Compare a create-stratum override with the lower default it shadows")
                .arg(path_arg()),
        )
}

fn path_arg() -> Arg {
    Arg::new(opt::PATH)
        .required(true)
        .value_name("PATH")
        .value_parser(clap::builder::OsStringValueParser::new())
        .help("Path within a StrataFS mount")
}

fn json_arg() -> Arg {
    Arg::new(opt::JSON)
        .long("json")
        .action(ArgAction::SetTrue)
        .help("Emit stable JSON")
}
