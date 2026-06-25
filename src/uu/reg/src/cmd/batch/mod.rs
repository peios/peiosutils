// Batch operations: `reg export` (dump a subtree) and `reg apply` (apply a
// batch atomically). See docs/reg-spec.md §6.
//
// JSON is the canonical, exact format; the text form is the human-facing
// default for `export`. `apply` auto-detects (leading `{`/`[` ⇒ JSON). The
// text *parser* is deferred until the §6.1 escaping grammar is finalised
// (apply of text input returns a clear error pointing there); text *export*
// is available now for human review.

pub mod apply;
pub mod export;

use serde::{Deserialize, Serialize};

/// A serialisable subtree snapshot — the JSON batch document.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub keys: Vec<KeyEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyEntry {
    pub path: String,
    #[serde(default)]
    pub values: Vec<ValueEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValueEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub data: serde_json::Value,
}
