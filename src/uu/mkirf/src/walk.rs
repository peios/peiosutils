//! Walk a source directory into the ordered list of cpio entries.

use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use glob::{MatchOptions, Pattern, PatternError};

use crate::cpio::{Body, Entry};

/// Compiled `--exclude` globs, matched against each entry's path relative to
/// the source root (e.g. `var/lib/peipkg/db.sqlite`).
///
/// Match options follow the modern, least-surprising convention: `*` and `?`
/// stay within a single path segment and `**` crosses separators, so
/// `var/lib/peipkg/*` excludes that directory's direct children while
/// `var/lib/peipkg/**` excludes the whole subtree. A pattern that matches a
/// directory prunes it and everything beneath it (the walk does not descend).
pub struct Excludes {
    patterns: Vec<Pattern>,
    options: MatchOptions,
}

impl Excludes {
    /// Compile the raw `--exclude` globs. A malformed glob is a usage error.
    pub fn compile(globs: &[String]) -> Result<Self, PatternError> {
        let patterns = globs
            .iter()
            .map(|g| Pattern::new(g))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            patterns,
            options: MatchOptions {
                case_sensitive: true,
                require_literal_separator: true,
                require_literal_leading_dot: false,
            },
        })
    }

    fn excluded(&self, rel: &Path) -> bool {
        self.patterns
            .iter()
            .any(|p| p.matches_path_with(rel, self.options))
    }
}

/// Collect every object beneath `root` into archive entries, skipping any path
/// matched by `excludes` (and not descending into an excluded directory).
///
/// `root` itself is not emitted — it is the archive's `/`. The returned
/// list is sorted by archive path in `LC_ALL=C` byte order, which places
/// every directory before its descendants: the order the kernel's
/// initramfs unpacker requires. See DESIGN.md §5.
pub fn walk(root: &Path, excludes: &Excludes) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    collect(root, root, excludes, &mut entries)?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn collect(dir: &Path, root: &Path, excludes: &Excludes, out: &mut Vec<Entry>) -> io::Result<()> {
    for child in fs::read_dir(dir).map_err(|e| at(e, dir))? {
        let child = child.map_err(|e| at(e, dir))?;
        let path = child.path();
        let rel = path
            .strip_prefix(root)
            .expect("child path is always beneath root");

        // An excluded path is dropped; an excluded directory also prunes its
        // whole subtree, since we never descend into it.
        if excludes.excluded(rel) {
            continue;
        }

        let meta = fs::symlink_metadata(&path).map_err(|e| at(e, &path))?;
        let ft = meta.file_type();

        let name = rel.as_os_str().as_bytes().to_vec();

        if ft.is_dir() {
            out.push(Entry {
                name,
                body: Body::Directory,
            });
            collect(&path, root, excludes, out)?;
        } else if ft.is_symlink() {
            let target = fs::read_link(&path).map_err(|e| at(e, &path))?;
            out.push(Entry {
                name,
                body: Body::Symlink {
                    target: target.as_os_str().as_bytes().to_vec(),
                },
            });
        } else if ft.is_file() {
            out.push(Entry {
                name,
                body: Body::File {
                    source: path,
                    size: meta.len(),
                    executable: meta.mode() & 0o111 != 0,
                },
            });
        } else if ft.is_char_device() || ft.is_block_device() {
            let rdev = meta.rdev();
            out.push(Entry {
                name,
                body: Body::Device {
                    block: ft.is_block_device(),
                    major: dev_major(rdev),
                    minor: dev_minor(rdev),
                },
            });
        } else if ft.is_fifo() {
            out.push(Entry {
                name,
                body: Body::Fifo,
            });
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{}: unsupported file type (socket?)", path.display()),
            ));
        }
    }
    Ok(())
}

/// Wrap an I/O error with the path on which it occurred.
fn at(e: io::Error, path: &Path) -> io::Error {
    io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}

// Linux device-number decoding, matching glibc's `gnu_dev_major`/`minor`.
fn dev_major(dev: u64) -> u64 {
    ((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff_u64)
}
fn dev_minor(dev: u64) -> u64 {
    (dev & 0xff) | ((dev >> 12) & !0xff_u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use tempfile::tempdir;

    fn names(entries: &[Entry]) -> Vec<String> {
        entries
            .iter()
            .map(|e| String::from_utf8_lossy(&e.name).into_owned())
            .collect()
    }

    fn no_excludes() -> Excludes {
        Excludes::compile(&[]).unwrap()
    }

    fn excludes(globs: &[&str]) -> Excludes {
        Excludes::compile(&globs.iter().map(|g| g.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn root_itself_is_not_emitted() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file"), b"x").unwrap();
        assert_eq!(names(&walk(dir.path(), &no_excludes()).unwrap()), ["file"]);
    }

    #[test]
    fn directories_precede_their_contents() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("usr/bin")).unwrap();
        fs::write(dir.path().join("usr/bin/sh"), b"").unwrap();
        fs::write(dir.path().join("init"), b"").unwrap();
        // LC_ALL=C byte order: every directory lands before its descendants.
        assert_eq!(
            names(&walk(dir.path(), &no_excludes()).unwrap()),
            ["init", "usr", "usr/bin", "usr/bin/sh"],
        );
    }

    #[test]
    fn executable_bit_is_detected() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("plain"), b"").unwrap();
        fs::write(dir.path().join("exe"), b"").unwrap();
        fs::set_permissions(dir.path().join("plain"), PermissionsExt::from_mode(0o644)).unwrap();
        fs::set_permissions(dir.path().join("exe"), PermissionsExt::from_mode(0o755)).unwrap();

        for e in walk(dir.path(), &no_excludes()).unwrap() {
            match (e.name.as_slice(), &e.body) {
                (b"exe", Body::File { executable, .. }) => assert!(*executable),
                (b"plain", Body::File { executable, .. }) => assert!(!*executable),
                _ => panic!("unexpected entry: {:?}", e.name),
            }
        }
    }

    #[test]
    fn symlink_target_is_captured() {
        let dir = tempdir().unwrap();
        symlink("../usr/bin/busybox", dir.path().join("sh")).unwrap();
        match &walk(dir.path(), &no_excludes()).unwrap()[0].body {
            Body::Symlink { target } => {
                assert_eq!(target.as_slice(), b"../usr/bin/busybox")
            }
            _ => panic!("expected a symlink entry"),
        }
    }

    #[test]
    fn sockets_are_rejected() {
        let dir = tempdir().unwrap();
        let _sock = std::os::unix::net::UnixListener::bind(dir.path().join("sock")).unwrap();
        let err = walk(dir.path(), &no_excludes()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    /// A directory exclude prunes the directory and its whole subtree, while a
    /// sibling at the same level is untouched.
    #[test]
    fn excluded_directory_is_pruned_with_its_subtree() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("var/lib/peipkg")).unwrap();
        fs::write(dir.path().join("var/lib/peipkg/db.sqlite"), b"x").unwrap();
        fs::create_dir_all(dir.path().join("var/lib/keep")).unwrap();
        fs::write(dir.path().join("var/lib/keep/state"), b"y").unwrap();

        let got = names(&walk(dir.path(), &excludes(&["var/lib/peipkg"])).unwrap());
        assert_eq!(
            got,
            ["var", "var/lib", "var/lib/keep", "var/lib/keep/state"]
        );
    }

    /// `*` stays within a path segment; `**` crosses separators.
    #[test]
    fn glob_separator_semantics() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("a/b")).unwrap();
        fs::write(dir.path().join("top.sqlite"), b"").unwrap();
        fs::write(dir.path().join("a/mid.sqlite"), b"").unwrap();
        fs::write(dir.path().join("a/b/deep.sqlite"), b"").unwrap();

        // `*.sqlite` matches only at the top level — `*` does not cross `/`.
        let top_only = names(&walk(dir.path(), &excludes(&["*.sqlite"])).unwrap());
        assert!(!top_only.contains(&"top.sqlite".to_string()));
        assert!(top_only.contains(&"a/mid.sqlite".to_string()));
        assert!(top_only.contains(&"a/b/deep.sqlite".to_string()));

        // `a/**` crosses separators: everything under `a/` is pruned at every
        // depth, while `a` itself and the root file remain.
        let crossed = names(&walk(dir.path(), &excludes(&["a/**"])).unwrap());
        assert_eq!(crossed, ["a", "top.sqlite"]);
    }

    #[test]
    fn malformed_glob_is_an_error() {
        assert!(Excludes::compile(&["a/[".to_string()]).is_err());
    }
}
