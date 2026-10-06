// The on-disk catalogue.
//
// Fragments live in one drop-in directory (`/usr/share/evman` by default).
// Each `*.evman` file is one component's fragment, and the component is the
// file stem (PGSS §6.10). A reader reads every fragment in the directory and
// no subdirectory. The directory is overridable by environment variable,
// which is also how the tests point evman at a tempdir.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::error::Result;

/// Default drop-in directory.
pub const DEFAULT_DIR: &str = "/usr/share/evman";

/// The corpus directory, honouring `EVMAN_DIR`.
pub fn dir() -> PathBuf {
    std::env::var_os("EVMAN_DIR").map_or_else(|| PathBuf::from(DEFAULT_DIR), PathBuf::from)
}

/// Fragment name for a fragment path (the file stem): `kacs` for
/// `/usr/share/evman/kacs.evman`. Rules 4 and 5 turn on it.
pub fn fragment_of(path: &Path) -> String {
    path.file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("?")
        .to_string()
}

/// Every `*.evman` fragment in the corpus, sorted by path for determinism.
pub fn fragments(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        // A missing corpus is not an error — there is simply nothing defined.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for entry in rd {
        let entry = entry?;
        let path = entry.path();
        // No subdirectory is read, even one named like a fragment.
        if path.extension().and_then(OsStr::to_str) == Some("evman") && entry.file_type()?.is_file()
        {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

/// One fragment's text and where it came from.
#[derive(Debug, Clone)]
pub struct Source {
    pub path: PathBuf,
    pub text: String,
}

impl Source {
    pub fn read(path: &Path) -> Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            text: std::fs::read_to_string(path)?,
        })
    }

    /// The fragment name: the file stem.
    pub fn fragment(&self) -> String {
        fragment_of(&self.path)
    }
}

/// Read every fragment in the corpus.
pub fn read(dir: &Path) -> Result<Vec<Source>> {
    fragments(dir)?.iter().map(|p| Source::read(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragment_is_file_stem() {
        assert_eq!(fragment_of(Path::new("/usr/share/evman/kacs.evman")), "kacs");
        assert_eq!(
            fragment_of(Path::new("/usr/share/evman/org.jellyfin.server.evman")),
            "org.jellyfin.server"
        );
    }

    #[test]
    fn missing_dir_yields_empty() {
        let p = PathBuf::from("/nonexistent/evman/dir/xyz");
        assert!(fragments(&p).unwrap().is_empty());
    }

    #[test]
    fn lists_only_evman_files_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("b.evman"), "x").unwrap();
        std::fs::write(tmp.path().join("a.evman"), "x").unwrap();
        std::fs::write(tmp.path().join("note.txt"), "x").unwrap();
        std::fs::write(tmp.path().join("kmes.regman"), "x").unwrap();
        let got: Vec<_> = fragments(tmp.path())
            .unwrap()
            .iter()
            .map(|p| fragment_of(p))
            .collect();
        assert_eq!(got, vec!["a", "b"]);
    }

    #[test]
    fn subdirectories_are_not_read() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.evman"), "x").unwrap();
        std::fs::create_dir(tmp.path().join("nested.evman")).unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub").join("c.evman"), "x").unwrap();
        let got: Vec<_> = fragments(tmp.path())
            .unwrap()
            .iter()
            .map(|p| fragment_of(p))
            .collect();
        assert_eq!(got, vec!["a"]);
    }

    #[test]
    fn read_returns_text_and_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("kmes.evman"), "--- field a.b\n").unwrap();
        let sources = read(tmp.path()).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].fragment(), "kmes");
        assert_eq!(sources[0].text, "--- field a.b\n");
    }
}
