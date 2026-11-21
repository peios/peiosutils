// attrib ~ (peiosutils) show or change the read-only attribute of files.
//
// KACS gives a file a read-only flag, kept in its special attributes
// (`security.peios.sattr`) and enforced for every process: a read-only
// regular file cannot be opened for writing or deleted, by anyone, its
// owner included, until the flag is cleared. It is the attribute Windows
// programs set and check, and the interface is Windows' `attrib`: `+r`
// sets it, `-r` clears it, and with neither each file's attributes are
// shown.
//
// Read-only is not a permission. It grants and denies no one anything;
// who may change a file is its security descriptor's business, and `sd` is
// the tool for that. Anyone with FILE_WRITE_ATTRIBUTES can clear the flag.
// On a directory the flag is kept and shown but not enforced, as on
// Windows.
//
// A symbolic link is followed, as `mkexec` follows one: the flag protects
// the file the link reaches.

use clap::{Arg, ArgAction, Command};
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use uucore::display::Quotable;
use uucore::error::{FromIo, UResult, UUsageError};
use uucore::show_if_err;
use uucore::special_attributes;

const ABOUT: &str = "Show or change the read-only attribute of files";
const USAGE: &str = "attrib [+r | -r] FILE...";
const AFTER_HELP: &str = "\
With +r, make each FILE read-only; with -r, make it writable again. With
neither, show each FILE's attributes: R if it is read-only, - if not.

A read-only regular file cannot be opened for writing or deleted, by anyone,
until the flag is cleared. A directory keeps the flag but is not protected by
it. A symbolic link is followed. A file named like a change can be given as
./-r.";

mod options {
    pub const OPERANDS: &str = "operands";
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;

    let operands: Vec<OsString> = matches
        .get_many::<OsString>(options::OPERANDS)
        .unwrap_or_default()
        .cloned()
        .collect();
    let (read_only, files) = parse_operands(&operands)?;

    if let Some(read_only) = read_only {
        for file in files {
            show_if_err!(set(Path::new(file), read_only));
        }
    } else {
        let mut stdout = io::stdout().lock();
        for file in files {
            show_if_err!(show(&mut stdout, file));
        }
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new("attrib")
        .version(uucore::crate_version!())
        .about(ABOUT)
        .override_usage(USAGE)
        .after_help(AFTER_HELP)
        .infer_long_args(true)
        .arg(
            Arg::new(options::OPERANDS)
                .required(true)
                .action(ArgAction::Append)
                // `-r` is a change, not a flag; clap has no option it
                // could be taken for, so it reaches the operands.
                .allow_hyphen_values(true)
                .value_name("FILE")
                .help("+r or -r, then the files to show or change")
                .value_parser(clap::value_parser!(OsString))
                .value_hint(clap::ValueHint::FilePath),
        )
}

/// What a change operand asks for: `Some(true)` for `+r`, `Some(false)`
/// for `-r`, and `None` if the operand is a file. The letter may be
/// either case, as Windows accepts.
fn change(operand: &OsStr) -> Option<bool> {
    match operand.as_bytes() {
        [b'+', b'r' | b'R'] => Some(true),
        [b'-', b'r' | b'R'] => Some(false),
        _ => None,
    }
}

/// Split the operands into the read-only change, if any, and the files.
/// Changes may come anywhere among the files, as Windows allows; asking
/// for both is an error.
fn parse_operands(operands: &[OsString]) -> UResult<(Option<bool>, Vec<&OsStr>)> {
    let mut read_only = None;
    let mut files = Vec::new();
    for operand in operands {
        match (change(operand), read_only) {
            (Some(wanted), Some(earlier)) if wanted != earlier => {
                return Err(UUsageError::new(1, "+r and -r cannot be given together"));
            }
            (Some(wanted), _) => read_only = Some(wanted),
            (None, _) => files.push(operand.as_os_str()),
        }
    }
    if files.is_empty() {
        return Err(UUsageError::new(1, "missing file operand"));
    }
    Ok((read_only, files))
}

/// Make one file read-only, or writable again.
fn set(path: &Path, read_only: bool) -> UResult<()> {
    special_attributes::set_read_only(path, read_only, true)
        .map_err_context(|| format!("cannot change the attributes of {}", path.quote()))
}

/// Show one file's attributes: `R` or `-`, two spaces, then its name as
/// given.
fn show(out: &mut impl Write, file: &OsStr) -> UResult<()> {
    let path = Path::new(file);
    let attrs = special_attributes::read(path, true)
        .map_err_context(|| format!("cannot read the attributes of {}", path.quote()))?;
    let mark: &[u8] = if attrs.read_only { b"R  " } else { b"-  " };
    out.write_all(mark)
        .and_then(|()| out.write_all(file.as_bytes()))
        .and_then(|()| out.write_all(b"\n"))
        .map_err_context(|| "write error".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operands(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn verifies_clap_config() {
        uu_app().debug_assert();
    }

    #[test]
    fn reads_a_change_in_either_case() {
        assert_eq!(change(OsStr::new("+r")), Some(true));
        assert_eq!(change(OsStr::new("+R")), Some(true));
        assert_eq!(change(OsStr::new("-r")), Some(false));
        assert_eq!(change(OsStr::new("-R")), Some(false));
    }

    #[test]
    fn takes_anything_else_for_a_file() {
        for name in ["r", "+rw", "-rf", "./-r", "+", "-", "--r"] {
            assert_eq!(change(OsStr::new(name)), None, "{name}");
        }
    }

    #[test]
    fn shows_without_a_change() {
        let ops = operands(&["a", "b"]);
        let (read_only, files) = parse_operands(&ops).unwrap();
        assert_eq!(read_only, None);
        assert_eq!(files, [OsStr::new("a"), OsStr::new("b")]);
    }

    #[test]
    fn takes_a_change_anywhere_among_the_files() {
        let ops = operands(&["a", "-r", "b"]);
        let (read_only, files) = parse_operands(&ops).unwrap();
        assert_eq!(read_only, Some(false));
        assert_eq!(files, [OsStr::new("a"), OsStr::new("b")]);
    }

    #[test]
    fn accepts_the_same_change_twice() {
        let ops = operands(&["+r", "a", "+R"]);
        assert_eq!(parse_operands(&ops).unwrap().0, Some(true));
    }

    #[test]
    fn refuses_both_changes() {
        let ops = operands(&["+r", "-r", "a"]);
        assert!(parse_operands(&ops).is_err());
    }

    #[test]
    fn needs_a_file() {
        let ops = operands(&["+r"]);
        assert!(parse_operands(&ops).is_err());
    }

    #[test]
    fn parses_minus_r_as_an_operand() {
        let matches = uu_app()
            .try_get_matches_from(["attrib", "-r", "file"])
            .unwrap();
        let ops: Vec<&OsString> = matches
            .get_many::<OsString>(options::OPERANDS)
            .unwrap()
            .collect();
        assert_eq!(ops, [&OsString::from("-r"), &OsString::from("file")]);
    }
}
