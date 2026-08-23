// Clap surface for `feat`.

use clap::{Arg, Command};

pub fn build() -> Command {
    let name_arg = || {
        Arg::new("name")
            .required(true)
            .help("Feature name (a directory under /libexec/features/)")
    };

    Command::new("feat")
        .version(uucore::crate_version!())
        .about("Install and enable Peios features — the imperative layer above packages")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("list").about("List available features and their state"))
        .subcommand(
            Command::new("install")
                .about("Run a feature's install.sh (does not enable it)")
                .arg(name_arg()),
        )
        .subcommand(
            Command::new("enable")
                .about("Run an installed feature's enable.sh")
                .arg(name_arg()),
        )
        .subcommand(
            Command::new("disable")
                .about("Run an enabled feature's disable.sh")
                .arg(name_arg()),
        )
        .subcommand(
            Command::new("add")
                .about("Install then enable a feature")
                .arg(name_arg()),
        )
        .subcommand(
            Command::new("remove")
                .about("Disable then uninstall a feature")
                .arg(name_arg()),
        )
        .subcommand(
            Command::new("uninstall")
                .about("Alias for `remove` (you cannot uninstall without disabling)")
                .arg(name_arg()),
        )
}
