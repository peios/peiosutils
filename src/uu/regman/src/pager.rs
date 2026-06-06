// Man-style paging and colour detection.
//
// When stdout is a terminal, output is piped through a pager so long pages
// scroll. We default to `less -FRX`: `-F` quits immediately if the page fits on
// one screen (so short entries print inline, like `man`), `-R` passes our ANSI
// styling through, `-X` leaves the text on screen. `$PAGER` overrides the
// choice. When stdout is not a terminal, we just write plain text.

use std::io::{IsTerminal, Write};
use std::process::{Command, Stdio};

/// ANSI styling is emitted only to a terminal, and never when `NO_COLOR` is set.
pub fn color_enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Write `text`, paging through a pager when stdout is a terminal. Falls back to
/// a plain write if there is no terminal or the pager can't be spawned.
pub fn emit(text: &str) {
    if std::io::stdout().is_terminal() && page(text).is_ok() {
        return;
    }
    let _ = std::io::stdout().write_all(text.as_bytes());
}

fn page(text: &str) -> std::io::Result<()> {
    let (program, args) = match std::env::var("PAGER") {
        Ok(p) if !p.trim().is_empty() => {
            let mut parts = p.split_whitespace().map(str::to_string);
            let program = parts.next().unwrap();
            (program, parts.collect::<Vec<_>>())
        }
        _ => ("less".to_string(), vec!["-FRX".to_string()]),
    };

    let mut cmd = Command::new(&program);
    cmd.args(&args).stdin(Stdio::piped());
    // Ensure a bare `less` (e.g. PAGER=less) still quits-on-one-screen and shows
    // colour, unless the user has their own LESS preferences.
    if program.ends_with("less") && std::env::var_os("LESS").is_none() {
        cmd.env("LESS", "FRX");
    }

    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }
    child.wait()?;
    Ok(())
}
