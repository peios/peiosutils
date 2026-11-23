// attrib ~ (peiosutils) show or change the read-only and not-indexed
// attributes of files.
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
//
// Not indexed is a file's PGSS Indexing preference (PGSS §8, PEI-1765):
// the `user.pgss.indexing` extended attribute, holding `all`,
// `no-content` or `none`, which tells a search indexer how far it may
// record the file, and for a directory everything beneath it. `+i`
// keeps a file's contents out of search, as Windows' `attrib +i` does,
// and `-i` removes the preference; `--indexing` sets any of the words.
// The listing shows only a file's own preference, not one it takes from
// a directory above.

use clap::{Arg, ArgAction, Command};
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use uucore::display::Quotable;
use uucore::error::{FromIo, UResult, USimpleError, UUsageError};
use uucore::show_if_err;
use uucore::special_attributes;

const ABOUT: &str = "Show or change the read-only and not-indexed attributes of files";
const USAGE: &str = "attrib [+r | -r] [+i | -i | --indexing WORD] FILE...";
const AFTER_HELP: &str = "\
With +r, make each FILE read-only; with -r, make it writable again. With +i,
keep each FILE's contents out of search; with -i, remove its indexing
preference. With no change, show each FILE's attributes: R if it is
read-only, then I if it has a preference that keeps it from being indexed in
full, each - if not.

A read-only regular file cannot be opened for writing or deleted, by anyone,
until the flag is cleared. A directory keeps the flag but is not protected by
it. A directory's indexing preference holds for everything beneath it. A
symbolic link is followed. A file named like a change can be given as ./-r.";

mod options {
    pub const OPERANDS: &str = "operands";
    pub const INDEXING: &str = "indexing";
}

/// PGSS Indexing's attribute and words (PGSS §8.2).
const INDEXING_ATTR: &str = "user.pgss.indexing";
const INDEXING_WORDS: [&str; 3] = ["all", "no-content", "none"];

/// A change to a file's indexing preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Indexing {
    /// `+i`: keep the contents out of search, leaving a stricter word.
    Exclude,
    /// `-i`: remove the preference.
    Remove,
    /// `--indexing`: set this word.
    Word(&'static str),
}

/// The changes the operands and options ask for.
#[derive(Debug, Default, PartialEq, Eq)]
struct Changes {
    read_only: Option<bool>,
    indexing: Option<Indexing>,
}

impl Changes {
    fn any(&self) -> bool {
        self.read_only.is_some() || self.indexing.is_some()
    }
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;

    let operands: Vec<OsString> = matches
        .get_many::<OsString>(options::OPERANDS)
        .unwrap_or_default()
        .cloned()
        .collect();
    let (mut changes, files) = parse_operands(&operands)?;
    if let Some(word) = matches.get_one::<String>(options::INDEXING) {
        if changes.indexing.is_some() {
            return Err(UUsageError::new(
                1,
                "--indexing cannot be given with +i or -i",
            ));
        }
        let word = INDEXING_WORDS
            .into_iter()
            .find(|w| w == word)
            .expect("clap allows only the words");
        changes.indexing = Some(Indexing::Word(word));
    }

    if changes.any() {
        for file in files {
            show_if_err!(set(Path::new(file), &changes));
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
            Arg::new(options::INDEXING)
                .long(options::INDEXING)
                .value_name("WORD")
                .help("Set each file's indexing preference: all, no-content or none")
                .value_parser(INDEXING_WORDS),
        )
        .arg(
            Arg::new(options::OPERANDS)
                .required(true)
                .action(ArgAction::Append)
                // `-r` is a change, not a flag; clap has no option it
                // could be taken for, so it reaches the operands.
                .allow_hyphen_values(true)
                .value_name("FILE")
                .help("+r, -r, +i or -i, then the files to show or change")
                .value_parser(clap::value_parser!(OsString))
                .value_hint(clap::ValueHint::FilePath),
        )
}

/// A change operand: an attribute letter, `r` or `i`, and whether it is
/// set (`+`) or cleared (`-`); `None` if the operand is a file. The letter
/// may be either case, as Windows accepts.
fn change(operand: &OsStr) -> Option<(u8, bool)> {
    match operand.as_bytes() {
        [sign @ (b'+' | b'-'), letter @ (b'r' | b'R' | b'i' | b'I')] => {
            Some((letter.to_ascii_lowercase(), *sign == b'+'))
        }
        _ => None,
    }
}

/// Split the operands into the changes they ask for and the files.
/// Changes may come anywhere among the files, as Windows allows; setting
/// and clearing one attribute together is an error.
fn parse_operands(operands: &[OsString]) -> UResult<(Changes, Vec<&OsStr>)> {
    let mut read_only = None;
    let mut indexed_out = None;
    let mut files = Vec::new();
    for operand in operands {
        let Some((letter, wanted)) = change(operand) else {
            files.push(operand.as_os_str());
            continue;
        };
        let (slot, both) = if letter == b'r' {
            (&mut read_only, "+r and -r cannot be given together")
        } else {
            (&mut indexed_out, "+i and -i cannot be given together")
        };
        if slot.is_some_and(|earlier| earlier != wanted) {
            return Err(UUsageError::new(1, both));
        }
        *slot = Some(wanted);
    }
    if files.is_empty() {
        return Err(UUsageError::new(1, "missing file operand"));
    }
    let indexing = indexed_out.map(|out| {
        if out {
            Indexing::Exclude
        } else {
            Indexing::Remove
        }
    });
    Ok((
        Changes {
            read_only,
            indexing,
        },
        files,
    ))
}

/// Make one change to one file.
fn set(path: &Path, changes: &Changes) -> UResult<()> {
    if let Some(read_only) = changes.read_only {
        special_attributes::set_read_only(path, read_only, true)
            .map_err_context(|| format!("cannot change the attributes of {}", path.quote()))?;
    }
    if let Some(indexing) = changes.indexing {
        set_indexing(path, indexing).map_err(|e| {
            if is_unsupported(&e) {
                USimpleError::new(
                    1,
                    format!(
                        "{}: this file system cannot keep an indexing preference",
                        path.maybe_quote()
                    ),
                )
            } else {
                e.map_err_context(|| {
                    format!("cannot change the indexing preference of {}", path.quote())
                })
            }
        })?;
    }
    Ok(())
}

fn is_unsupported(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(libc::EOPNOTSUPP))
}

/// A file's own indexing word, if it has one.
fn indexing(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match xattr::get(path, INDEXING_ATTR) {
        Err(e) if is_unsupported(&e) => Ok(None),
        other => other,
    }
}

/// Whether a file's own word keeps it from being indexed in full: any
/// word but `all`, as PGSS §8.2 reads an unknown one as `no-content`.
fn not_indexed(word: Option<&[u8]>) -> bool {
    word.is_some_and(|w| w.strip_suffix(&[0]).unwrap_or(w) != b"all")
}

fn set_indexing(path: &Path, change: Indexing) -> io::Result<()> {
    match change {
        // A file already kept from full indexing keeps its stricter word.
        Indexing::Exclude if not_indexed(indexing(path)?.as_deref()) => Ok(()),
        Indexing::Exclude => xattr::set(path, INDEXING_ATTR, b"no-content"),
        Indexing::Word(word) => xattr::set(path, INDEXING_ATTR, word.as_bytes()),
        Indexing::Remove => match xattr::remove(path, INDEXING_ATTR) {
            Err(e) if e.raw_os_error() == Some(libc::ENODATA) || is_unsupported(&e) => Ok(()),
            other => other,
        },
    }
}

/// Show one file's attributes: `R` or `-`, a space, `I` or `-`, two
/// spaces, then its name as given.
fn show(out: &mut impl Write, file: &OsStr) -> UResult<()> {
    let path = Path::new(file);
    let attrs = special_attributes::read(path, true)
        .map_err_context(|| format!("cannot read the attributes of {}", path.quote()))?;
    let word = indexing(path)
        .map_err_context(|| format!("cannot read the attributes of {}", path.quote()))?;
    let mark = [
        if attrs.read_only { b'R' } else { b'-' },
        b' ',
        if not_indexed(word.as_deref()) {
            b'I'
        } else {
            b'-'
        },
        b' ',
        b' ',
    ];
    out.write_all(&mark)
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
        assert_eq!(change(OsStr::new("+r")), Some((b'r', true)));
        assert_eq!(change(OsStr::new("+R")), Some((b'r', true)));
        assert_eq!(change(OsStr::new("-r")), Some((b'r', false)));
        assert_eq!(change(OsStr::new("-R")), Some((b'r', false)));
        assert_eq!(change(OsStr::new("+I")), Some((b'i', true)));
        assert_eq!(change(OsStr::new("-i")), Some((b'i', false)));
    }

    #[test]
    fn takes_anything_else_for_a_file() {
        for name in ["r", "+rw", "-rf", "./-r", "+", "-", "--r", "+ri", "-x"] {
            assert_eq!(change(OsStr::new(name)), None, "{name}");
        }
    }

    #[test]
    fn shows_without_a_change() {
        let ops = operands(&["a", "b"]);
        let (changes, files) = parse_operands(&ops).unwrap();
        assert!(!changes.any());
        assert_eq!(files, [OsStr::new("a"), OsStr::new("b")]);
    }

    #[test]
    fn takes_a_change_anywhere_among_the_files() {
        let ops = operands(&["a", "-r", "b", "+i"]);
        let (changes, files) = parse_operands(&ops).unwrap();
        assert_eq!(changes.read_only, Some(false));
        assert_eq!(changes.indexing, Some(Indexing::Exclude));
        assert_eq!(files, [OsStr::new("a"), OsStr::new("b")]);
    }

    #[test]
    fn accepts_the_same_change_twice() {
        let ops = operands(&["+r", "a", "+R"]);
        assert_eq!(parse_operands(&ops).unwrap().0.read_only, Some(true));
    }

    #[test]
    fn refuses_both_changes() {
        let ops = operands(&["+r", "-r", "a"]);
        assert!(parse_operands(&ops).is_err());
        let ops = operands(&["+i", "a", "-I"]);
        assert!(parse_operands(&ops).is_err());
    }

    #[test]
    fn any_word_but_all_keeps_a_file_from_full_indexing() {
        assert!(!not_indexed(None));
        assert!(!not_indexed(Some(b"all")));
        assert!(not_indexed(Some(b"no-content")));
        assert!(not_indexed(Some(b"none")));
        assert!(not_indexed(Some(b"typo")));
    }

    #[test]
    fn takes_only_the_standards_words() {
        assert!(
            uu_app()
                .try_get_matches_from(["attrib", "--indexing", "none", "f"])
                .is_ok()
        );
        assert!(
            uu_app()
                .try_get_matches_from(["attrib", "--indexing", "some", "f"])
                .is_err()
        );
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
