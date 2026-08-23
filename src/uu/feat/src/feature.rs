// Feature definitions on disk: a read-only directory per feature, holding up to
// four lifecycle scripts. Discovery, name validation, and script execution.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};

/// Vendor feature library. Definitions are shipped here (as plain files) by
/// `feat-<name>` packages; feat only reads and runs them.
pub const FEATURES_DIR: &str = "/libexec/features";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Install,
    Enable,
    Disable,
    Uninstall,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Install => "install",
            Phase::Enable => "enable",
            Phase::Disable => "disable",
            Phase::Uninstall => "uninstall",
        }
    }

    fn script_file(self) -> String {
        format!("{}.sh", self.as_str())
    }
}

/// Validate a feature name: a single path component, no separators or traversal.
/// Keeps a name from escaping FEATURES_DIR.
pub fn validate_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !name.contains('/');
    if ok {
        Ok(())
    } else {
        Err(Error::Usage(format!("invalid feature name: {name:?}")))
    }
}

pub fn feature_dir(name: &str) -> PathBuf {
    Path::new(FEATURES_DIR).join(name)
}

/// A feature exists iff its directory exists.
pub fn exists(name: &str) -> bool {
    feature_dir(name).is_dir()
}

/// List the feature names present in FEATURES_DIR, sorted. A missing directory
/// yields an empty list (no features installed on this image).
pub fn list() -> Result<Vec<String>> {
    let entries = match std::fs::read_dir(FEATURES_DIR) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(Error::Io {
                op: format!("read {FEATURES_DIR}"),
                source,
            });
        }
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            op: format!("read entry in {FEATURES_DIR}"),
            source,
        })?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Run a feature's lifecycle script for `phase`. A missing script is a no-op
/// (returns Ok), so a feature only ships the phases it actually needs.
///
/// The script runs as a plain child: it inherits the caller's token (no
/// escalation), the caller's stdio, and a copy of the environment plus
/// `FEAT_NAME`/`FEAT_DIR`/`FEAT_PHASE`. Its cwd is the feature directory so it
/// can reference sibling files. A non-zero exit aborts the operation; the caller
/// must not record a state change for a failed script.
pub fn run_phase(name: &str, phase: Phase) -> Result<()> {
    let dir = feature_dir(name);
    let script = dir.join(phase.script_file());
    if !script.is_file() {
        return Ok(());
    }

    let status = Command::new(&script)
        .current_dir(&dir)
        .env("FEAT_NAME", name)
        .env("FEAT_DIR", &dir)
        .env("FEAT_PHASE", phase.as_str())
        .status()
        .map_err(|source| Error::Io {
            op: format!("run {}", script.display()),
            source,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::Script {
            feature: name.to_string(),
            phase: phase.as_str(),
            code: status.code(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{feature_dir, validate_name, FEATURES_DIR};

    #[test]
    fn feature_library_is_opened_through_the_runtime_view() {
        assert_eq!(FEATURES_DIR, "/libexec/features");
        assert_eq!(
            feature_dir("dynamic-boot"),
            Path::new(FEATURES_DIR).join("dynamic-boot")
        );
    }

    #[test]
    fn accepts_ordinary_names() {
        for name in ["foo", "foo-bar", "net_base", "x11.core", "a1"] {
            assert!(validate_name(name).is_ok(), "{name} should be valid");
        }
    }

    #[test]
    fn rejects_traversal_and_separators() {
        for name in ["", ".", "..", "../evil", "a/b", "/abs", "a\\b", "white space"] {
            assert!(validate_name(name).is_err(), "{name:?} should be rejected");
        }
    }
}
