// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Clap surface for `mount` (spec §2.5). Path/value args are `OsString` so
//! non-UTF-8 operands survive losslessly (§1.8).

use std::ffi::OsString;

use clap::{Arg, ArgAction, Command};

/// Stable arg ids.
pub mod opt {
    pub const TYPE: &str = "types";
    pub const OPTIONS: &str = "options";
    pub const READ_ONLY: &str = "read-only";
    pub const READ_WRITE: &str = "read-write";
    pub const VERBOSE: &str = "verbose";
    pub const FAKE: &str = "fake";
    pub const OPERANDS: &str = "operands";
    pub const SOURCE: &str = "source";
    pub const TARGET: &str = "target";
    pub const TARGET_PREFIX: &str = "target-prefix";
    pub const BIND: &str = "bind";
    pub const RBIND: &str = "rbind";
    pub const MOVE: &str = "move";
    pub const BENEATH: &str = "beneath";
    pub const MAKE_SHARED: &str = "make-shared";
    pub const MAKE_SLAVE: &str = "make-slave";
    pub const MAKE_PRIVATE: &str = "make-private";
    pub const MAKE_UNBINDABLE: &str = "make-unbindable";
    pub const MAKE_RSHARED: &str = "make-rshared";
    pub const MAKE_RSLAVE: &str = "make-rslave";
    pub const MAKE_RPRIVATE: &str = "make-rprivate";
    pub const MAKE_RUNBINDABLE: &str = "make-runbindable";
    pub const EXCLUSIVE: &str = "exclusive";
    pub const MKDIR: &str = "mkdir";
    pub const NO_CANONICALIZE: &str = "no-canonicalize";
    pub const SHOW_LABELS: &str = "show-labels";
    pub const ONLYONCE: &str = "onlyonce";
    pub const NAMESPACE: &str = "namespace";
    pub const INTERNAL_ONLY: &str = "internal-only";
    pub const NO_MTAB: &str = "no-mtab";
    pub const SYNTH_SDDL: &str = "synth-sddl";
    pub const LABEL: &str = "label";
    pub const UUID: &str = "uuid";
}

fn osstr(name: &'static str) -> Arg {
    Arg::new(name).value_parser(clap::value_parser!(OsString))
}

fn boolean(name: &'static str) -> Arg {
    Arg::new(name).action(ArgAction::SetTrue)
}

/// Build the `mount` clap Command.
pub fn build() -> Command {
    Command::new("mount")
        .version(uucore::crate_version!())
        .about("Attach a filesystem to the Peios mount tree")
        .long_about(
            "Attach a filesystem via the fd-based mount API \
             (fsopen/fsconfig/fsmount/move_mount). Mount policy is \
             superblock-scoped. See docs/mount-spec.md.",
        )
        .arg(osstr(opt::TYPE).short('t').long("types").value_name("TYPE").help("Filesystem type (or 'auto')"))
        .arg(
            osstr(opt::OPTIONS)
                .short('o')
                .long("options")
                .value_name("LIST")
                .action(ArgAction::Append)
                .help("Comma-separated mount options; repeatable"),
        )
        .arg(
            boolean(opt::READ_ONLY)
                .short('r')
                .long("read-only")
                .visible_alias("ro")
                .overrides_with(opt::READ_WRITE)
                .help("Mount read-only (-o ro)"),
        )
        .arg(
            boolean(opt::READ_WRITE)
                .short('w')
                .long("rw")
                .visible_alias("read-write")
                .overrides_with(opt::READ_ONLY)
                .help("Mount read-write and forbid the auto-ro fallback (-o rw)"),
        )
        .arg(osstr(opt::SOURCE).long("source").value_name("SRC").help("Explicit source operand"))
        .arg(osstr(opt::TARGET).long("target").value_name("DIR").help("Explicit target operand"))
        .arg(
            osstr(opt::TARGET_PREFIX)
                .long("target-prefix")
                .value_name("DIR")
                .help("Prepend DIR to the target path"),
        )
        .arg(boolean(opt::BIND).short('B').long("bind").help("Bind a subtree elsewhere"))
        .arg(boolean(opt::RBIND).short('R').long("rbind").help("Bind a subtree and all submounts"))
        .arg(boolean(opt::MOVE).short('M').long("move").help("Relocate an existing mount"))
        .arg(boolean(opt::BENEATH).long("beneath").help("Mount beneath the current top mount at the target"))
        .arg(boolean(opt::MAKE_SHARED).long("make-shared").help("Mark a subtree shared"))
        .arg(boolean(opt::MAKE_SLAVE).long("make-slave").help("Mark a subtree slave"))
        .arg(boolean(opt::MAKE_PRIVATE).long("make-private").help("Mark a subtree private"))
        .arg(boolean(opt::MAKE_UNBINDABLE).long("make-unbindable").help("Mark a subtree unbindable"))
        .arg(boolean(opt::MAKE_RSHARED).long("make-rshared").help("Recursively mark a subtree shared"))
        .arg(boolean(opt::MAKE_RSLAVE).long("make-rslave").help("Recursively mark a subtree slave"))
        .arg(boolean(opt::MAKE_RPRIVATE).long("make-rprivate").help("Recursively mark a subtree private"))
        .arg(boolean(opt::MAKE_RUNBINDABLE).long("make-runbindable").help("Recursively mark a subtree unbindable"))
        .arg(boolean(opt::EXCLUSIVE).long("exclusive").help("Force a unique superblock instance (no reuse)"))
        .arg(
            Arg::new(opt::MKDIR)
                .short('m')
                .long("mkdir")
                .value_name("MODE")
                .num_args(0..=1)
                .default_missing_value("0755")
                .help("Create the target directory if missing (default mode 0755)"),
        )
        .arg(
            boolean(opt::NO_CANONICALIZE)
                .short('c')
                .long("no-canonicalize")
                .help("Do not canonicalize paths"),
        )
        .arg(boolean(opt::FAKE).short('f').long("fake").help("Dry run: skip the mount syscalls"))
        .arg(
            Arg::new(opt::VERBOSE)
                .short('v')
                .long("verbose")
                .action(ArgAction::Count)
                .help("Verbose; drain the kernel fs_context log"),
        )
        .arg(boolean(opt::SHOW_LABELS).short('l').long("show-labels").help("Show filesystem labels in listings"))
        .arg(boolean(opt::ONLYONCE).long("onlyonce").help("Skip if already mounted"))
        .arg(osstr(opt::NAMESPACE).short('N').long("namespace").value_name("NS").help("Operate in mount namespace NS"))
        .arg(boolean(opt::INTERNAL_ONLY).short('i').long("internal-only").help("Do not invoke a mount.<type> helper"))
        .arg(boolean(opt::NO_MTAB).short('n').long("no-mtab").help("Accepted and ignored (no mtab on peios)"))
        .arg(osstr(opt::SYNTH_SDDL).long("synth-sddl").value_name("SDDL").help("KACS synth-policy template SD"))
        .arg(osstr(opt::LABEL).short('L').long("label").value_name("LABEL").help("Mount the filesystem with this label (LABEL=)"))
        .arg(osstr(opt::UUID).short('U').long("uuid").value_name("UUID").help("Mount the filesystem with this UUID (UUID=)"))
        .arg(
            // util-linux accepts options interspersed with the operands
            // (e.g. `mount SRC -o OPTS TGT`). Append lets clap collect the
            // operands across such option-split groups; resolve_operands then
            // validates the count (§4).
            osstr(opt::OPERANDS)
                .action(ArgAction::Append)
                .num_args(1)
                .value_name("SOURCE|TARGET")
                .help("SOURCE and TARGET (or a lone TARGET for remount / --make-*)"),
        )
}
