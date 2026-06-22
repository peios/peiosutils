// Output helpers. Human-readable by default; structured JSON via the
// `Json` value type returned by every subcommand. The CLI dispatcher
// decides which view to print based on `--json`.

use crate::sid_render::{self, SidStyle};
use peios::security::Sid;
use peios::token::TokenAccess;

/// One token-handle access-mask bit → name.
pub fn access_mask_names(mask: u32) -> String {
    let pairs: &[(u32, &str)] = &[
        (TokenAccess::ASSIGN_PRIMARY.bits(), "ASSIGN_PRIMARY"),
        (TokenAccess::DUPLICATE.bits(), "DUPLICATE"),
        (TokenAccess::IMPERSONATE.bits(), "IMPERSONATE"),
        (TokenAccess::QUERY.bits(), "QUERY"),
        (TokenAccess::QUERY_SOURCE.bits(), "QUERY_SOURCE"),
        (TokenAccess::ADJUST_PRIVS.bits(), "ADJUST_PRIVS"),
        (TokenAccess::ADJUST_GROUPS.bits(), "ADJUST_GROUPS"),
        (TokenAccess::ADJUST_DEFAULT.bits(), "ADJUST_DEFAULT"),
        (TokenAccess::ADJUST_SESSIONID.bits(), "ADJUST_SESSIONID"),
    ];
    let names: Vec<&str> = pairs
        .iter()
        .filter_map(|(bit, name)| (mask & bit != 0).then_some(*name))
        .collect();
    if names.is_empty() {
        format!("0x{mask:x}")
    } else {
        format!("0x{:x} ({})", mask, names.join(" | "))
    }
}

/// A line emitted in the human-readable layout. The dispatcher renders
/// `Lines` to stdout with key/value alignment.
#[derive(Debug, Clone)]
pub enum Line {
    /// Section header (printed bold-ish by trailing newline before).
    Section(String),
    /// `key: value` pair, aligned.
    Kv(String, String),
    /// Free-form indented detail line under a Kv.
    Detail(String),
    /// Blank line.
    Blank,
}

/// A buffer of human-readable lines being assembled by a command.
#[derive(Default, Debug, Clone)]
pub struct Lines(pub Vec<Line>);

impl Lines {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn section(&mut self, s: impl Into<String>) {
        self.0.push(Line::Section(s.into()));
    }
    pub fn kv(&mut self, k: impl Into<String>, v: impl Into<String>) {
        self.0.push(Line::Kv(k.into(), v.into()));
    }
    pub fn detail(&mut self, s: impl Into<String>) {
        self.0.push(Line::Detail(s.into()));
    }
    pub fn blank(&mut self) {
        self.0.push(Line::Blank);
    }

    /// Convenience: render a SID under a key.
    pub fn sid(&mut self, key: impl Into<String>, sid: &Sid, style: SidStyle) {
        self.kv(key, sid_render::render(sid, style));
    }

    /// Print to stdout with key column alignment.
    pub fn print(&self) {
        let max_key = self
            .0
            .iter()
            .filter_map(|l| match l {
                Line::Kv(k, _) => Some(k.len()),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        for line in &self.0 {
            match line {
                Line::Section(s) => println!("\n[{s}]"),
                Line::Kv(k, v) => {
                    println!("  {k:<width$}  {v}", width = max_key);
                }
                Line::Detail(s) => println!("      {s}"),
                Line::Blank => println!(),
            }
        }
    }
}

/// Output mode picked by the CLI dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
}

/// Generic command output. Commands return both shapes; the dispatcher
/// picks one to print.
pub struct CmdOutput {
    pub human: Lines,
    pub json: serde_json::Value,
}

impl CmdOutput {
    pub fn print(&self, mode: OutputMode) {
        match mode {
            OutputMode::Human => self.human.print(),
            OutputMode::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&self.json)
                        .unwrap_or_else(|_| "{}".into())
                );
            }
        }
    }
}
