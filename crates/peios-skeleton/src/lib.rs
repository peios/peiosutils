//! The Peios root skeleton file (PSPU 11): what a root needs outside `/usr`,
//! which no package can ship, as one entry per line.
//!
//! ```text
//! # A comment.
//! /var
//! /var/state
//! /var/state/secrets O:SYG:SYD:P(A;OICI;GA;;;SY)
//! /lib64 -> usr/lib/x86_64-linux-peios
//! ```
//!
//! An entry is a directory, with an optional security descriptor in SDDL, or
//! a symbolic link, `path -> target`. fsbase ships the file at
//! `/usr/share/skeleton`, and whatever builds a root reads the one in that
//! root's own `/usr` and applies it: live-root at boot, peiso for the
//! initramfs, the installer and the upgrade tool.
//!
//! [`parse`] reads a file whole, so that a file with any error changes
//! nothing. [`apply`] makes what is missing, in the file's order, and never
//! deletes or changes what is there. Applying twice changes nothing.
//!
//! This crate has no dependencies, so that a root builder off Peios, such as
//! peiso, can read the file too. Creating a directory is the caller's: on
//! Peios, the `skeleton` utility creates each one with its descriptor in the
//! same call, never creating it and then setting the descriptor; where a
//! tree can't hold descriptors, as a cpio archive can't, the caller creates
//! it plainly.

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

/// Where a root holds its skeleton file, relative to the root.
pub const PATH: &str = "usr/share/skeleton";

/// One entry of a skeleton file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A directory, and the security descriptor, in SDDL, to create it with.
    /// Without one, it takes what it inherits from its parent.
    Dir {
        path: String,
        descriptor: Option<String>,
    },
    /// A symbolic link, and its target, relative to the link's directory.
    Symlink { path: String, target: String },
}

impl Entry {
    /// The entry's absolute path in the root, as the file gives it.
    pub fn path(&self) -> &str {
        match self {
            Self::Dir { path, .. } | Self::Symlink { path, .. } => path,
        }
    }
}

/// A read skeleton file: its entries in the file's order, each after its
/// parent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Skeleton {
    pub entries: Vec<Entry>,
}

/// Why a skeleton file can't be read: the line, from 1, and what is wrong
/// with it.
#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Reads the skeleton file `text`. Any error rejects the whole file.
pub fn parse(text: &str) -> Result<Skeleton, ParseError> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut dirs = HashSet::new();
    for (index, line) in text.split('\n').enumerate() {
        let error = |message: String| ParseError {
            line: index + 1,
            message,
        };
        let line = line.trim_matches(BLANK);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(BLANK).filter(|f| !f.is_empty()).collect();
        let entry = match fields[..] {
            [path] => Entry::Dir {
                path: path.to_owned(),
                descriptor: None,
            },
            [path, "->"] => return Err(error(format!("{path}: a link needs a target"))),
            [path, descriptor] => Entry::Dir {
                path: path.to_owned(),
                descriptor: Some(descriptor.to_owned()),
            },
            [path, "->", target] => Entry::Symlink {
                path: path.to_owned(),
                target: target.to_owned(),
            },
            _ => {
                return Err(error(
                    "not `path`, `path descriptor` or `path -> target`".to_owned(),
                ));
            }
        };
        let path = entry.path();
        check_path(path).map_err(|why| error(format!("{path}: {why}")))?;
        if !seen.insert(path.to_owned()) {
            return Err(error(format!("{path}: listed twice")));
        }
        let parent = parent(path);
        if parent != "/" && !dirs.contains(parent) {
            return Err(error(format!(
                "{path}: its parent, {parent}, isn't a directory listed before it"
            )));
        }
        match &entry {
            Entry::Dir { .. } => {
                dirs.insert(path.to_owned());
            }
            Entry::Symlink { target, .. } => {
                check_target(path, target).map_err(|why| error(format!("{path}: {why}")))?;
            }
        }
        entries.push(entry);
    }
    Ok(Skeleton { entries })
}

/// What separates a line's fields.
const BLANK: [char; 2] = [' ', '\t'];

/// Whether `path` is one an entry may have: absolute, in canonical form, and
/// outside `/usr`.
fn check_path(path: &str) -> Result<(), &'static str> {
    let Some(rest) = path.strip_prefix('/') else {
        return Err("not an absolute path");
    };
    if rest.is_empty() {
        return Err("the root itself is not an entry");
    }
    if rest.split('/').any(|part| matches!(part, "" | "." | "..")) {
        return Err("not in canonical form: an empty, `.` or `..` component");
    }
    if rest == "usr" || rest.starts_with("usr/") {
        return Err("/usr is the packages', not the skeleton's");
    }
    Ok(())
}

/// Whether `target` is one the link at `path` may have: relative, and
/// resolving inside the root wherever the root is.
fn check_target(path: &str, target: &str) -> Result<(), &'static str> {
    if target.starts_with('/') {
        return Err("a link's target must be relative to the link's directory");
    }
    let mut depth = parent(path)
        .split('/')
        .filter(|part| !part.is_empty())
        .count();
    for part in target.split('/') {
        match part {
            "" => return Err("a link's target has an empty component"),
            "." => {}
            ".." => {
                depth = depth
                    .checked_sub(1)
                    .ok_or("a link's target leaves the root")?;
            }
            _ => depth += 1,
        }
    }
    Ok(())
}

/// The directory `path` is in, `/` for one at the top.
fn parent(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) | None => "/",
        Some(at) => &path[..at],
    }
}

/// What applying a skeleton did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// The entries made, in order.
    pub created: Vec<String>,
    /// How many were already there.
    pub present: usize,
}

/// Why applying a skeleton stopped: the entry it stopped at, and why. What
/// was made before it stays, and applying again continues from there.
#[derive(Debug)]
pub struct ApplyError {
    pub path: String,
    pub error: io::Error,
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.error)
    }
}

impl std::error::Error for ApplyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Applies `skeleton` to the root at `root`: makes each entry that is
/// missing, in order, and leaves each that is there as it is. Something of
/// another kind in an entry's place, or a link to another target, is an
/// error. Nothing is followed: each entry's parent is the root or an entry
/// already found to be a directory, so no entry is made through a link.
///
/// `make_dir(path, descriptor)` creates the directory `path`, which must
/// not exist, with `descriptor` if it is given.
pub fn apply(
    root: &Path,
    skeleton: &Skeleton,
    mut make_dir: impl FnMut(&Path, Option<&str>) -> io::Result<()>,
) -> Result<Applied, ApplyError> {
    let mut applied = Applied::default();
    for entry in &skeleton.entries {
        let at = in_root(root, entry.path());
        let fail = |error: io::Error| ApplyError {
            path: entry.path().to_owned(),
            error,
        };
        let found = match fs::symlink_metadata(&at) {
            Ok(found) => Some(found.file_type()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(fail(error)),
        };
        match (entry, found) {
            (Entry::Dir { descriptor, .. }, None) => {
                make_dir(&at, descriptor.as_deref()).map_err(fail)?;
            }
            (Entry::Symlink { target, .. }, None) => symlink(target, &at).map_err(fail)?,
            (Entry::Dir { .. }, Some(kind)) if kind.is_dir() => {
                applied.present += 1;
                continue;
            }
            (Entry::Symlink { target, .. }, Some(kind)) if kind.is_symlink() => {
                let has = fs::read_link(&at).map_err(fail)?;
                if has != Path::new(target) {
                    return Err(fail(other(format!(
                        "a link to {} is in the way of the link to {target}",
                        has.display()
                    ))));
                }
                applied.present += 1;
                continue;
            }
            (entry, Some(kind)) => {
                let want = match entry {
                    Entry::Dir { .. } => "directory",
                    Entry::Symlink { .. } => "link",
                };
                return Err(fail(other(format!(
                    "{} is in the way of the {want}",
                    kind_name(kind)
                ))));
            }
        }
        applied.created.push(entry.path().to_owned());
    }
    Ok(applied)
}

/// Where the entry at `path` is under `root`.
fn in_root(root: &Path, path: &str) -> PathBuf {
    root.join(path.trim_start_matches('/'))
}

fn other(message: String) -> io::Error {
    io::Error::other(message)
}

fn kind_name(kind: fs::FileType) -> &'static str {
    if kind.is_dir() {
        "a directory"
    } else if kind.is_symlink() {
        "a link"
    } else if kind.is_file() {
        "a file"
    } else {
        "a special file"
    }
}

#[cfg(test)]
mod tests;
