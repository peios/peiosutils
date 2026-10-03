// regman ~ (peiosutils) — the Peios registry manual.
//
// `regman <path> [value]` explains what a registry key or value does. It is a
// documentation tool, tangential to the registry: it reads on-disk fragments
// under /usr/share/regman and never touches LCS or the live registry.
//
// See `peios/regman-design.md` for the full design.
//
// Reading the manual is libregman's, which programs other than this take
// too; this is the command line and the terminal's rendering.

use clap::Command;
use uucore::error::{UResult, USimpleError};

pub use libregman::{corpus, error, fold, fragment, index, pattern, query, scan};

pub mod cli;
pub mod cmd;
pub mod markdown;
pub mod pager;
pub mod render;
pub mod watch;

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let cli = cli::build();
    let matches = match cli.try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            let code = e.exit_code();
            e.print().ok();
            return if code == 0 {
                Ok(())
            } else {
                Err(USimpleError::new(code, ""))
            };
        }
    };
    match cmd::dispatch(&matches) {
        Ok(()) => Ok(()),
        Err(err) => {
            let code = err.exit_code();
            Err(USimpleError::new(code, err.to_string()))
        }
    }
}

pub fn uu_app() -> Command {
    cli::build()
}
