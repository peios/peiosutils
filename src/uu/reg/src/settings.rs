// Behavioural settings, resolved from flags + environment (docs/reg-spec.md
// §4.0). Precedence: explicit flag > environment variable > built-in default.

use crate::error::{Error, Result};
use std::io::IsTerminal;

/// The resolved output/behaviour knobs shared across subcommands.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Separator used when *displaying* key paths.
    pub sep: char,
    /// Emit structured JSON instead of human text.
    pub json: bool,
    /// Verbose output.
    pub verbose: bool,
    /// Suppress non-essential output.
    pub quiet: bool,
    /// Skip confirmation prompts for destructive ops.
    pub assume_yes: bool,
    /// Default target layer for writes (`None` = base layer).
    pub layer: Option<String>,
}

fn env_flag(name: &str) -> bool {
    matches!(std::env::var(name).ok().as_deref(), Some("1" | "true" | "yes"))
}

impl Settings {
    /// Resolve settings from a subcommand's matches plus the environment.
    pub fn from_matches(m: &clap::ArgMatches) -> Result<Settings> {
        let sep = match m.get_one::<String>("sep") {
            Some(s) => parse_sep(s)?,
            None => match std::env::var("REG_SEP").ok().as_deref() {
                Some(s) if !s.is_empty() => parse_sep(s)?,
                _ => '\\',
            },
        };
        let json = m.get_flag("json") || env_flag("REG_JSON");
        let verbose = m.get_flag("verbose") || env_flag("REG_VERBOSE");
        let quiet = m.get_flag("quiet");
        // `yes` is only defined on the destructive subcommands (via yes_arg());
        // the read commands don't carry it, so get_flag("yes") would panic on an
        // undefined arg. try_get_one tolerates its absence (→ false).
        let assume_yes = m
            .try_get_one::<bool>("yes")
            .ok()
            .flatten()
            .copied()
            .unwrap_or(false)
            || env_flag("REG_ASSUME_YES");
        let layer = m
            .get_one::<String>("layer")
            .cloned()
            .or_else(|| std::env::var("REG_LAYER").ok().filter(|s| !s.is_empty()));
        Ok(Settings {
            sep,
            json,
            verbose,
            quiet,
            assume_yes,
            layer,
        })
    }

    /// The target layer for a write as the libpeios ABI wants it: `None` is the
    /// base layer.
    pub fn layer_arg(&self) -> Option<&str> {
        self.layer.as_deref()
    }

    /// A human label for the active layer, for echo lines.
    pub fn layer_label(&self) -> &str {
        self.layer.as_deref().unwrap_or("base")
    }

    /// Confirm a destructive action. Returns `Ok(true)` to proceed. Auto-yes
    /// when `--yes`/`REG_ASSUME_YES` is set or when stdin is not a TTY (so
    /// scripts are never blocked); otherwise prompts on stderr.
    pub fn confirm(&self, prompt: &str) -> Result<bool> {
        if self.assume_yes || !std::io::stdin().is_terminal() {
            return Ok(true);
        }
        use std::io::Write;
        eprint!("{prompt} [y/N] ");
        std::io::stderr().flush().ok();
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .map_err(|e| Error::Syscall {
                op: "read confirmation",
                errno: e.raw_os_error().unwrap_or(5),
                detail: None,
            })?;
        Ok(matches!(line.trim(), "y" | "Y" | "yes" | "Yes"))
    }
}

fn parse_sep(s: &str) -> Result<char> {
    match s {
        "\\" | "backslash" => Ok('\\'),
        "/" | "slash" => Ok('/'),
        other => Err(Error::Usage(format!(
            "invalid separator {other:?}: expected '/' or '\\'"
        ))),
    }
}
