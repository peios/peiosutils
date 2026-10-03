// The on-disk documentation corpus.
//
// Fragments live in a drop-in directory (`/usr/share/regman` by default). Each
// `*.regman` file is one provider; the provider name is the file stem (design
// §3). Both the corpus dir and the index path are overridable by environment
// variable, which is also how the tests point regman at a tempdir.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::error::Result;

/// Default drop-in directory.
pub const DEFAULT_DIR: &str = "/usr/share/regman";
/// Default index location (a cache, not under the read-only corpus).
pub const DEFAULT_INDEX: &str = "/var/cache/regman/index";

/// The corpus directory, honouring `REGMAN_DIR`.
pub fn dir() -> PathBuf {
    std::env::var_os("REGMAN_DIR").map_or_else(|| PathBuf::from(DEFAULT_DIR), PathBuf::from)
}

/// The index file path, honouring `REGMAN_INDEX`.
pub fn index_path() -> PathBuf {
    std::env::var_os("REGMAN_INDEX").map_or_else(|| PathBuf::from(DEFAULT_INDEX), PathBuf::from)
}

/// Provider name for a fragment path (the file stem).
pub fn provider_of(path: &Path) -> String {
    path.file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("?")
        .to_string()
}

/// Every `*.regman` fragment in the corpus, sorted by path for determinism.
pub fn fragments(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        // A missing corpus is not an error — there is simply nothing documented.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for entry in rd {
        let path = entry?.path();
        if path.extension().and_then(OsStr::to_str) == Some("regman") {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_is_file_stem() {
        assert_eq!(provider_of(Path::new("/usr/share/regman/kmes.regman")), "kmes");
    }

    #[test]
    fn missing_dir_yields_empty() {
        let p = PathBuf::from("/nonexistent/regman/dir/xyz");
        assert!(fragments(&p).unwrap().is_empty());
    }

    #[test]
    fn lists_only_regman_files_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("b.regman"), "x").unwrap();
        std::fs::write(tmp.path().join("a.regman"), "x").unwrap();
        std::fs::write(tmp.path().join("note.txt"), "x").unwrap();
        std::fs::write(tmp.path().join(".index"), "x").unwrap();
        let got: Vec<_> = fragments(tmp.path())
            .unwrap()
            .iter()
            .map(|p| provider_of(p))
            .collect();
        assert_eq!(got, vec!["a", "b"]);
    }
}
