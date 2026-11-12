// skeleton ~ (peiosutils) apply a Peios root skeleton file.
//
// A package holds only /usr, so what a root needs outside it -- /var, /lcl,
// /tmp and the rest, and the psABI's /lib64 link -- is described by a root
// skeleton file (PSPU 11), which fsbase ships at /usr/share/skeleton. Whatever
// builds a root applies the file its own /usr holds: live-root at boot, and
// the installer and the upgrade tool. `skeleton apply ROOT` is how they do it.
//
// The file is read whole, and every descriptor in it parsed, before anything
// is made, so a file with any error changes nothing. Then each entry is made
// if it is missing, in the file's order, and left alone if it is there.
// Something of another kind in an entry's place is an error. Applying twice
// changes nothing.
//
// A directory with a descriptor is created with it, in one native create
// (kacs_open), never created and then given it: there is no moment at which
// it has the descriptor it would inherit instead. That create rolls back if
// the new descriptor grants its creator no data right, so whoever applies a
// skeleton must be granted one by each descriptor in it; the skeleton's
// descriptors all grant SYSTEM full control, and root builders run as SYSTEM.
// A directory without one is created plainly, and inherits from its parent.

use clap::{Arg, ArgAction, Command};
use peios::file::{CreateOptions, Disposition, FileAccess, OpenFlags, OpenOptions};
use peios::security::{SecurityDescriptor, sddl};
use peios_skeleton::{Entry, Skeleton};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use uucore::display::Quotable;
use uucore::error::{UResult, USimpleError};

const ABOUT: &str = "Apply a Peios root skeleton file to a root";

mod options {
    pub const APPLY: &str = "apply";
    pub const ROOT: &str = "root";
    pub const FILE: &str = "file";
    pub const VERBOSE: &str = "verbose";
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    let Some(apply) = matches.subcommand_matches(options::APPLY) else {
        unreachable!("clap requires a subcommand");
    };
    let root = PathBuf::from(
        apply
            .get_one::<OsString>(options::ROOT)
            .expect("clap requires the root"),
    );
    let file = match apply.get_one::<OsString>(options::FILE) {
        Some(file) => PathBuf::from(file),
        None => root.join(peios_skeleton::PATH),
    };
    let verbose = apply.get_flag(options::VERBOSE);
    run(&root, &file, verbose)
}

pub fn uu_app() -> Command {
    Command::new("skeleton")
        .version(uucore::crate_version!())
        .about(ABOUT)
        .infer_long_args(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new(options::APPLY)
                .about("Make what a root's skeleton file lists and the root lacks")
                .arg(
                    Arg::new(options::ROOT)
                        .required(true)
                        .value_name("ROOT")
                        .help("The root to apply the skeleton to")
                        .value_parser(clap::value_parser!(OsString))
                        .value_hint(clap::ValueHint::DirPath),
                )
                .arg(
                    Arg::new(options::FILE)
                        .long(options::FILE)
                        .value_name("FILE")
                        .help("Apply FILE rather than the root's own usr/share/skeleton")
                        .value_parser(clap::value_parser!(OsString))
                        .value_hint(clap::ValueHint::FilePath),
                )
                .arg(
                    Arg::new(options::VERBOSE)
                        .short('v')
                        .long(options::VERBOSE)
                        .action(ArgAction::SetTrue)
                        .help("Print each entry made"),
                ),
        )
}

fn run(root: &Path, file: &Path, verbose: bool) -> UResult<()> {
    let text = fs::read_to_string(file)
        .map_err(|e| USimpleError::new(1, format!("cannot read {}: {e}", file.quote())))?;
    let skeleton = peios_skeleton::parse(&text)
        .map_err(|e| USimpleError::new(1, format!("{}: {e}", file.quote())))?;
    let descriptors = descriptors(&skeleton)
        .map_err(|e| USimpleError::new(1, format!("{}: {e}", file.quote())))?;
    let applied = peios_skeleton::apply(root, &skeleton, |at, descriptor| match descriptor {
        Some(sddl) => create_with(at, &descriptors[sddl]),
        None => fs::create_dir(at),
    })
    .map_err(|e| {
        USimpleError::new(
            1,
            format!("cannot apply {} to {}: {e}", file.quote(), root.quote()),
        )
    })?;
    if verbose {
        for path in &applied.created {
            println!("{path}");
        }
    }
    Ok(())
}

/// Each distinct descriptor the skeleton gives, parsed, by its SDDL.
fn descriptors(skeleton: &Skeleton) -> Result<HashMap<&str, SecurityDescriptor>, String> {
    let mut parsed = HashMap::new();
    for entry in &skeleton.entries {
        if let Entry::Dir {
            path,
            descriptor: Some(text),
        } = entry
            && !parsed.contains_key(text.as_str())
        {
            let sd = sddl::parse(text).map_err(|e| format!("{path}: invalid SDDL: {e}"))?;
            parsed.insert(text.as_str(), sd);
        }
    }
    Ok(parsed)
}

/// Creates the directory `at`, which must not exist, with `sd`, in one
/// native create. Not following a link at `at`, so that one swapped in
/// is an error, not a way to create elsewhere.
fn create_with(at: &Path, sd: &SecurityDescriptor) -> io::Result<()> {
    OpenOptions::new()
        // Every native open asks for a data right; for a directory, listing.
        .desired_access(FileAccess::LIST_DIRECTORY)
        .disposition(Disposition::Create)
        .options(CreateOptions::DIRECTORY)
        .flags(OpenFlags::SYMLINK_NOFOLLOW)
        .creator_sd(sd)
        .create(None, at)
        .map(drop)
        .map_err(|e| match e.raw_os_error() {
            Some(errno) => io::Error::from_raw_os_error(errno),
            None => io::Error::other(e.to_string()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_clap_config() {
        uu_app().debug_assert();
    }

    #[test]
    fn needs_apply_and_a_root() {
        assert!(uu_app().try_get_matches_from(["skeleton"]).is_err());
        assert!(
            uu_app()
                .try_get_matches_from(["skeleton", "apply"])
                .is_err()
        );
        let matches = uu_app()
            .try_get_matches_from(["skeleton", "apply", "--file", "f", "-v", "/mnt/root"])
            .unwrap();
        let apply = matches.subcommand_matches(options::APPLY).unwrap();
        assert_eq!(
            apply.get_one::<OsString>(options::ROOT).unwrap(),
            "/mnt/root"
        );
        assert_eq!(apply.get_one::<OsString>(options::FILE).unwrap(), "f");
        assert!(apply.get_flag(options::VERBOSE));
    }

    #[test]
    fn a_skeleton_without_descriptors_applies_anywhere() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("skeleton");
        fs::write(&file, "/var\n/var/log\n/lib64 -> usr/lib\n").unwrap();
        run(root.path(), &file, false).unwrap();
        run(root.path(), &file, false).unwrap();
        assert!(root.path().join("var/log").is_dir());
        assert_eq!(
            fs::read_link(root.path().join("lib64")).unwrap(),
            Path::new("usr/lib")
        );
    }

    #[test]
    fn a_bad_file_changes_nothing() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("skeleton");
        fs::write(&file, "/var\n/var/log\nlib64\n").unwrap();
        let error = run(root.path(), &file, false).unwrap_err();
        assert!(error.to_string().contains("line 3"), "{error}");
        assert!(!root.path().join("var").exists());
    }

    #[test]
    fn a_missing_file_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let error = run(root.path(), &root.path().join(peios_skeleton::PATH), false).unwrap_err();
        assert!(error.to_string().contains("cannot read"), "{error}");
    }
}
