// mkexec ~ (peiosutils) mark a regular file executable.
//
// Peios authorises through KACS security descriptors rather than POSIX
// mode, so `chmod` is not shipped at all and `install` is deferred until
// the Peios-native attribute model settles. One scrap of the POSIX mode
// still matters even so: the kernel refuses to execute a file carrying no
// execute bit anywhere, and that check runs in the DAC layer, before any
// LSM is consulted. It is not waived for SYSTEM either -- DAC_OVERRIDE
// grants execute only where some execute bit is already set. A file that
// arrives without one (unpacked by a transport that dropped it, written
// by a tool that never set it) is therefore unrunnable, and until now
// nothing on a Peios system could make it runnable again.
//
// So the bit is not a permission here; it is an intrinsic property of the
// file, meaning "this is a program". mkexec sets that mark and does
// nothing else -- it grants no access, because access is the security
// descriptor's business and `sd` is the tool for it.
//
// Which of the three slots carries the mark has no meaning under KACS:
// the mark is one property, and `ls` reports it when any slot is set. So
// all three are set together rather than picking a slot arbitrarily, and
// every other mode bit is left exactly as it was.

use clap::{Arg, ArgAction, Command};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use uucore::display::Quotable;
use uucore::error::{FromIo, UResult, USimpleError};
use uucore::show_if_err;

const ABOUT: &str = "Mark a regular file executable";

/// Every execute slot. Which one carries the mark is meaningless under
/// KACS, so the mark is written to all of them and read from any.
pub const EXEC_MARK: u32 = 0o111;

mod options {
    pub const FILES: &str = "files";
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;

    let files = matches
        .get_many::<OsString>(options::FILES)
        .unwrap_or_default();
    for file in files {
        show_if_err!(mkexec(Path::new(file)));
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new("mkexec")
        .version(uucore::crate_version!())
        .about(ABOUT)
        .infer_long_args(true)
        .arg(
            Arg::new(options::FILES)
                .required(true)
                .action(ArgAction::Append)
                .value_name("FILE")
                .help("The regular file to mark executable")
                .value_parser(clap::value_parser!(OsString))
                .value_hint(clap::ValueHint::FilePath),
        )
}

/// Mark one path executable.
///
/// Symlinks are followed, so marking a link marks what it points at --
/// the mark belongs to the file that will be executed, and a symlink's
/// own mode is never consulted by the kernel.
///
/// Marking an already-marked file succeeds without touching it, so a
/// script that marks the same file twice is not an error.
fn mkexec(path: &Path) -> UResult<()> {
    let metadata =
        fs::metadata(path).map_err_context(|| format!("cannot mark {}", path.quote()))?;

    if !metadata.is_file() {
        let what = if metadata.is_dir() {
            "is a directory"
        } else {
            "is not a regular file"
        };
        return Err(USimpleError::new(
            1,
            format!(
                "cannot mark {}: {what}; the executable mark applies to regular files",
                path.quote()
            ),
        ));
    }

    let mode = metadata.permissions().mode();
    if mode & EXEC_MARK != 0 {
        return Ok(());
    }

    fs::set_permissions(path, fs::Permissions::from_mode(mode | EXEC_MARK))
        .map_err_context(|| format!("cannot mark {}", path.quote()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    fn mode_of(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o7777
    }

    fn write_file(dir: &Path, name: &str, mode: u32) -> std::path::PathBuf {
        let path = dir.join(name);
        File::create(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    #[test]
    fn verifies_clap_config() {
        uu_app().debug_assert();
    }

    #[test]
    fn marks_a_plain_file() {
        let dir = tempdir().unwrap();
        let file = write_file(dir.path(), "prog", 0o644);

        mkexec(&file).unwrap();

        assert_eq!(mode_of(&file), 0o755);
    }

    #[test]
    fn sets_every_execute_slot() {
        let dir = tempdir().unwrap();
        let file = write_file(dir.path(), "prog", 0o600);

        mkexec(&file).unwrap();

        assert_eq!(mode_of(&file) & EXEC_MARK, EXEC_MARK);
    }

    #[test]
    fn preserves_the_other_mode_bits() {
        let dir = tempdir().unwrap();
        let file = write_file(dir.path(), "prog", 0o640);

        mkexec(&file).unwrap();

        // Read and write bits are untouched; only the mark is added.
        assert_eq!(mode_of(&file) & 0o666, 0o640);
    }

    #[test]
    fn is_idempotent() {
        let dir = tempdir().unwrap();
        let file = write_file(dir.path(), "prog", 0o755);

        mkexec(&file).unwrap();
        mkexec(&file).unwrap();

        assert_eq!(mode_of(&file), 0o755);
    }

    #[test]
    fn leaves_a_partial_mark_alone() {
        // Already executable through one slot: the file is marked, and
        // rewriting the mode would be a change with no meaning.
        let dir = tempdir().unwrap();
        let file = write_file(dir.path(), "prog", 0o601);

        mkexec(&file).unwrap();

        assert_eq!(mode_of(&file), 0o601);
    }

    #[test]
    fn refuses_a_directory() {
        let dir = tempdir().unwrap();

        let err = mkexec(dir.path()).unwrap_err();

        assert!(err.to_string().contains("is a directory"));
    }

    #[test]
    fn reports_a_missing_file() {
        let dir = tempdir().unwrap();

        let err = mkexec(&dir.path().join("absent")).unwrap_err();

        assert!(err.to_string().contains("cannot mark"));
    }

    #[test]
    fn follows_a_symlink_to_its_target() {
        let dir = tempdir().unwrap();
        let target = write_file(dir.path(), "prog", 0o644);
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        mkexec(&link).unwrap();

        assert_eq!(mode_of(&target) & EXEC_MARK, EXEC_MARK);
    }
}
