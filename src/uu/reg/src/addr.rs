// Addressing: key paths and value names (docs/reg-spec.md §2).
//
// A key path is a positional argument using `/` or `\` as a separator (neither
// is legal inside an LCS key component, so accepting both is unambiguous). The
// value name is a *separate* positional: absent ⇒ the command targets the key
// itself; the literal `@` ⇒ the default (empty-name) value.

use crate::error::{Error, Result};

/// A parsed, canonicalised registry key path (e.g. `Machine\System\App`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPath {
    /// Path components, in order, with separators removed. Never empty.
    components: Vec<String>,
}

impl KeyPath {
    /// Parse a CLI path string. Accepts `/` and `\` (and mixes of both) as
    /// separators; a leading separator is ignored; repeated separators
    /// collapse. Empty paths and zero-length components are rejected.
    pub fn parse(raw: &str) -> Result<KeyPath> {
        let components: Vec<String> = raw
            .split(['/', '\\'])
            .filter(|c| !c.is_empty())
            .map(|c| c.to_string())
            .collect();
        if components.is_empty() {
            return Err(Error::InvalidSpec(format!("empty key path: {raw:?}")));
        }
        Ok(KeyPath { components })
    }

    /// The path string handed to the libpeios ABI: backslash-joined, which is
    /// the on-the-wire LCS convention regardless of how the user typed it.
    pub fn to_abi(&self) -> String {
        self.components.join("\\")
    }

    /// Render the path for display using `sep` as the separator.
    pub fn display(&self, sep: char) -> String {
        self.components.join(&sep.to_string())
    }

    /// The leaf (last) component.
    pub fn leaf(&self) -> &str {
        self.components.last().expect("KeyPath is never empty")
    }

    /// The parent path, or `None` if this is a single (hive-root) component.
    pub fn parent(&self) -> Option<KeyPath> {
        if self.components.len() <= 1 {
            None
        } else {
            Some(KeyPath {
                components: self.components[..self.components.len() - 1].to_vec(),
            })
        }
    }

    /// Append a child component (used when walking subkeys).
    pub fn child(&self, name: &str) -> KeyPath {
        let mut components = self.components.clone();
        components.push(name.to_string());
        KeyPath { components }
    }
}

/// What a command targets: the key itself, or a named value within it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueTarget {
    /// No value argument was given — the command operates on the key.
    Key,
    /// A named value. Empty bytes = the default (`@`) value.
    Value(Vec<u8>),
}

impl ValueTarget {
    /// Build from the optional value-name positional. `@` selects the default
    /// (empty-name) value; any other string is taken as length-counted bytes.
    pub fn from_arg(arg: Option<&str>) -> ValueTarget {
        match arg {
            None => ValueTarget::Key,
            Some("@") => ValueTarget::Value(Vec::new()),
            Some(s) => ValueTarget::Value(s.as_bytes().to_vec()),
        }
    }

    /// The value-name bytes, or `None` if this targets the key.
    pub fn name_bytes(&self) -> Option<&[u8]> {
        match self {
            ValueTarget::Key => None,
            ValueTarget::Value(b) => Some(b),
        }
    }
}

/// Render a value name (bytes) for display: the default value as `(default)`,
/// otherwise a lossy UTF-8 rendering.
pub fn display_value_name(name: &[u8]) -> String {
    if name.is_empty() {
        "(default)".to_string()
    } else {
        String::from_utf8_lossy(name).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_both_separators() {
        let a = KeyPath::parse("Machine/System/App").unwrap();
        let b = KeyPath::parse(r"Machine\System\App").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.to_abi(), r"Machine\System\App");
        assert_eq!(a.display('/'), "Machine/System/App");
    }

    #[test]
    fn parse_collapses_and_strips() {
        let p = KeyPath::parse("/Machine//System/").unwrap();
        assert_eq!(p.to_abi(), r"Machine\System");
        assert_eq!(p.leaf(), "System");
        assert_eq!(p.parent().unwrap().to_abi(), "Machine");
    }

    #[test]
    fn empty_path_rejected() {
        assert!(KeyPath::parse("").is_err());
        assert!(KeyPath::parse("///").is_err());
    }

    #[test]
    fn value_target_default_and_named() {
        assert_eq!(ValueTarget::from_arg(None), ValueTarget::Key);
        assert_eq!(
            ValueTarget::from_arg(Some("@")),
            ValueTarget::Value(Vec::new())
        );
        assert_eq!(
            ValueTarget::from_arg(Some("Theme")),
            ValueTarget::Value(b"Theme".to_vec())
        );
    }
}
