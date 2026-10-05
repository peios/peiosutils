// Feature definitions on disk: a read-only directory per feature, holding up to
// four lifecycle scripts and an optional feature.toml that says what the
// feature is. Discovery, name validation, metadata, and script execution.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::{Error, Result};
use crate::report::Report;

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
    pub const ALL: [Phase; 4] = [Phase::Install, Phase::Enable, Phase::Disable, Phase::Uninstall];

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

/// The phases a feature ships a script for, in lifecycle order.
pub fn phases(name: &str) -> Vec<Phase> {
    let dir = feature_dir(name);
    Phase::ALL
        .into_iter()
        .filter(|phase| dir.join(phase.script_file()).is_file())
        .collect()
}

/// What a feature says it is, from the optional `feature.toml` in its
/// directory:
///
/// ```toml
/// title = "Dynamic Boot"
/// description = """
/// Keeps the boot image up to date..."""
/// ```
///
/// Both members are optional, and members feat does not know are ignored,
/// so a definition can carry more for a later feat.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub title: Option<String>,
    /// Paragraphs separated by one blank line, each on one line: the line
    /// breaks within a paragraph in the file are only where it was wrapped.
    pub description: Option<String>,
}

pub const METADATA_FILE: &str = "feature.toml";

/// Read a feature's metadata. No `feature.toml` is no metadata; one that
/// cannot be read or understood is an error in words, for the caller to show
/// beside the feature rather than refuse to list it.
pub fn metadata(name: &str) -> std::result::Result<Metadata, String> {
    let path = feature_dir(name).join(METADATA_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_metadata(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Metadata::default()),
        Err(e) => Err(format!("{METADATA_FILE}: {e}")),
    }
}

fn parse_metadata(text: &str) -> std::result::Result<Metadata, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| format!("{METADATA_FILE}: {}", e.message()))?;
    let member = |key: &str| -> std::result::Result<Option<String>, String> {
        match table.get(key) {
            None => Ok(None),
            Some(toml::Value::String(s)) if s.trim().is_empty() => Ok(None),
            Some(toml::Value::String(s)) => Ok(Some(s.trim().to_string())),
            Some(_) => Err(format!("{METADATA_FILE}: {key} must be a string")),
        }
    };
    Ok(Metadata {
        title: member("title")?,
        description: member("description")?.map(|d| reflow(&d)),
    })
}

/// Join each paragraph's wrapped lines into one, keeping paragraphs apart.
fn reflow(text: &str) -> String {
    text.split("\n\n")
        .map(|para| para.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|para| !para.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Run a feature's lifecycle script for `phase`. A missing script is a no-op
/// (returns Ok), so a feature only ships the phases it actually needs.
///
/// The script runs as a plain child: it inherits the caller's token (no
/// escalation), the caller's stdio, and a copy of the environment plus
/// `FEAT_NAME`/`FEAT_DIR`/`FEAT_PHASE`. Its cwd is the feature directory so it
/// can reference sibling files. A non-zero exit aborts the operation; the caller
/// must not record a state change for a failed script.
///
/// Driven, the script's standard output and error are read line by line and
/// sent as message events instead, and its standard input is empty: feat's
/// own is the program driving it, which is not the script's to read.
pub fn run_phase(name: &str, phase: Phase, report: Report) -> Result<()> {
    let dir = feature_dir(name);
    let script = dir.join(phase.script_file());
    if !script.is_file() {
        return Ok(());
    }

    let mut command = Command::new(&script);
    command
        .current_dir(&dir)
        .env("FEAT_NAME", name)
        .env("FEAT_DIR", &dir)
        .env("FEAT_PHASE", phase.as_str());
    let run_error = |source| Error::Io {
        op: format!("run {}", script.display()),
        source,
    };
    let status = if report.driven() {
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(run_error)?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        std::thread::scope(|scope| {
            if let Some(stderr) = stderr {
                scope.spawn(move || relay(stderr, report));
            }
            if let Some(stdout) = stdout {
                relay(stdout, report);
            }
        });
        child.wait().map_err(run_error)?
    } else {
        command.status().map_err(run_error)?
    };

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

/// Send each line a script writes as a message event, until it closes.
fn relay(from: impl Read, report: Report) {
    let mut lines = BufReader::new(from);
    let mut line = Vec::new();
    loop {
        line.clear();
        match lines.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                let text = String::from_utf8_lossy(&line);
                let text = text.trim_end();
                if !text.is_empty() {
                    report.output(text);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{feature_dir, parse_metadata, validate_name, Metadata, FEATURES_DIR};

    #[test]
    fn metadata_is_optional_member_by_member() {
        assert_eq!(parse_metadata("").unwrap(), Metadata::default());
        let only_title = parse_metadata("title = \"Dynamic Boot\"\nlater = 1\n").unwrap();
        assert_eq!(only_title.title.as_deref(), Some("Dynamic Boot"));
        assert_eq!(only_title.description, None);
    }

    #[test]
    fn a_description_is_reflowed_into_paragraphs() {
        let m = parse_metadata(
            "description = \"\"\"\nKeeps the boot image\nup to date.\n\nTakes effect at\nthe next boot.\n\"\"\"\n",
        )
        .unwrap();
        assert_eq!(
            m.description.as_deref(),
            Some("Keeps the boot image up to date.\n\nTakes effect at the next boot.")
        );
    }

    #[test]
    fn bad_metadata_is_said_in_words() {
        assert!(parse_metadata("title = 3\n").unwrap_err().contains("title must be a string"));
        assert!(parse_metadata("title = \n").unwrap_err().starts_with("feature.toml: "));
    }

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
