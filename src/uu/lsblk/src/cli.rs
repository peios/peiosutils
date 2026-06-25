// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Clap surface for `lsblk`. A subset of util-linux lsblk: the four output
//! modes, the column presets, and the common device filters. Hardware columns
//! that depend on a udev database (rather than sysfs) are out of v1 scope.

use clap::{Arg, ArgAction, Command};

/// Stable arg ids.
pub mod opt {
    pub const OUTPUT: &str = "output";
    pub const OUTPUT_ALL: &str = "output-all";
    pub const FS: &str = "fs";
    pub const PERMS: &str = "perms";
    pub const JSON: &str = "json";
    pub const PAIRS: &str = "pairs";
    pub const RAW: &str = "raw";
    pub const LIST: &str = "list";
    pub const ASCII: &str = "ascii";
    pub const BYTES: &str = "bytes";
    pub const NOHEADINGS: &str = "noheadings";
    pub const PATHS: &str = "paths";
    pub const NODEPS: &str = "nodeps";
    pub const ALL: &str = "all";
    pub const INVERSE: &str = "inverse";
    pub const INCLUDE: &str = "include";
    pub const EXCLUDE: &str = "exclude";
    pub const SYSROOT: &str = "sysroot";
    pub const DEVICE: &str = "device";
    pub const TREE: &str = "tree";
    pub const MERGE: &str = "merge";
    pub const DEDUP: &str = "dedup";
    pub const SORT: &str = "sort";
    pub const SHELL: &str = "shell";
    pub const WIDTH: &str = "width";
}

fn boolean(name: &'static str) -> Arg {
    Arg::new(name).action(ArgAction::SetTrue)
}

/// Build the `lsblk` clap Command.
pub fn build() -> Command {
    Command::new("lsblk")
        .version(uucore::crate_version!())
        .about("List information about block devices")
        .long_about(
            "List block devices as a tree (default), flat list, JSON, or \
             key=value pairs. Topology and hardware columns come from sysfs; \
             filesystem columns from libblkid; OWNER/MODE from each device \
             node's Security Descriptor (rendered like `ls -l`).",
        )
        .arg(
            Arg::new(opt::OUTPUT)
                .short('o')
                .long("output")
                .value_name("LIST")
                .help("Comma-separated columns to output"),
        )
        .arg(
            boolean(opt::OUTPUT_ALL)
                .short('O')
                .long("output-all")
                .help("Output all available columns"),
        )
        .arg(boolean(opt::FS).short('f').long("fs").help("Output filesystem information"))
        .arg(
            boolean(opt::PERMS)
                .short('m')
                .long("perms")
                .help("Output owner and SD-derived mode (mirrors `ls -l`)"),
        )
        .arg(boolean(opt::JSON).short('J').long("json").help("Use JSON output format"))
        .arg(
            boolean(opt::PAIRS)
                .short('P')
                .long("pairs")
                .help("Use key=\"value\" output format"),
        )
        .arg(boolean(opt::RAW).short('r').long("raw").help("Use raw output format"))
        .arg(boolean(opt::LIST).short('l').long("list").help("Use list output format"))
        .arg(
            boolean(opt::ASCII)
                .short('i')
                .long("ascii")
                .help("Use ascii characters for the tree"),
        )
        .arg(boolean(opt::BYTES).short('b').long("bytes").help("Print SIZE in bytes"))
        .arg(
            boolean(opt::NOHEADINGS)
                .short('n')
                .long("noheadings")
                .help("Don't print headings"),
        )
        .arg(
            boolean(opt::PATHS)
                .short('p')
                .long("paths")
                .help("Print full device paths"),
        )
        .arg(
            boolean(opt::NODEPS)
                .short('d')
                .long("nodeps")
                .help("Don't print slaves or holders"),
        )
        .arg(
            boolean(opt::ALL)
                .short('a')
                .long("all")
                .help("Print all devices, including empty ones"),
        )
        .arg(
            boolean(opt::INVERSE)
                .short('s')
                .long("inverse")
                .help("Print dependencies in inverse order"),
        )
        .arg(
            Arg::new(opt::INCLUDE)
                .short('I')
                .long("include")
                .value_name("LIST")
                .help("Show only devices with the given major numbers"),
        )
        .arg(
            Arg::new(opt::EXCLUDE)
                .short('e')
                .long("exclude")
                .value_name("LIST")
                .help("Exclude devices by major number (default: 1 / RAM disks)"),
        )
        .arg(
            Arg::new(opt::SYSROOT)
                .long("sysroot")
                .value_name("DIR")
                .help("Read sysfs, mountinfo and /dev from DIR instead of /"),
        )
        .arg(
            Arg::new(opt::TREE)
                .short('T')
                .long("tree")
                .num_args(0..=1)
                .default_missing_value("NAME")
                .value_name("COLUMN")
                .help("Force tree output, optionally with the tree on COLUMN"),
        )
        .arg(
            boolean(opt::MERGE)
                .short('M')
                .long("merge")
                .help("Group parents of sub-trees (e.g. multipath devices)"),
        )
        .arg(
            Arg::new(opt::DEDUP)
                .short('E')
                .long("dedup")
                .value_name("COLUMN")
                .help("De-duplicate output by COLUMN"),
        )
        .arg(
            Arg::new(opt::SORT)
                .short('x')
                .long("sort")
                .value_name("COLUMN")
                .help("Sort output by COLUMN"),
        )
        .arg(
            boolean(opt::SHELL)
                .short('y')
                .long("shell")
                .help("Use shell-compatible column names (- and : become _)"),
        )
        .arg(
            Arg::new(opt::WIDTH)
                .short('w')
                .long("width")
                .value_name("NUM")
                .help("Truncate table output to NUM columns wide"),
        )
        .arg(
            Arg::new(opt::DEVICE)
                .action(ArgAction::Append)
                .value_name("DEVICE")
                .help("Limit output to the given device(s)"),
        )
}
