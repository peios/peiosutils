// regman ~ (peiosutils) — the Peios registry manual.
//
// `regman <path> [value]` explains what a registry key or value does. It is a
// documentation tool, tangential to the registry: it reads on-disk fragments
// under /usr/share/regman and never touches LCS or the live registry.
//
// See `peios/regman-design.md` for the full design.

use clap::Command;
use uucore::error::{UResult, USimpleError};

pub mod cli;
pub mod cmd;
pub mod corpus;
pub mod error;
pub mod fold;
pub mod fragment;
pub mod index;
pub mod markdown;
pub mod pager;
pub mod query;
pub mod render;
pub mod scan;
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
