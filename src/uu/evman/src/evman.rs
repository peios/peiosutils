// evman ~ (peiosutils) — the Peios event manual.
//
// `evman <name>` explains what an event type or a field means: the event
// catalogue (PGSS §6.10) rendered for a terminal. It is a documentation tool,
// tangential to the event stream: it reads the on-disk fragments under
// /usr/share/evman and never reads a record.
//
// evman is to events what regman is to the registry, and is built the same
// way: reading and checking the catalogue is libevman's, which programs other
// than this take too; this is the command line and the terminal's rendering.

use clap::Command;
use uucore::error::{UResult, USimpleError};

pub use libevman::{catalogue, corpus, error, fragment, lint};

pub mod cli;
pub mod cmd;
pub mod markdown;
pub mod pager;
pub mod render;

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
