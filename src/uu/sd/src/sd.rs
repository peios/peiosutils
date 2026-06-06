// sd ~ (peiosutils) — entry point.
//
// See `peios/sd-design.md` for the full design.

use clap::Command;
use uucore::error::{UResult, USimpleError};

pub mod cli;
pub mod cmd;
pub mod error;
pub mod flags;
pub mod perms;
pub mod principal;
pub mod render;
pub mod target;
pub mod walk;

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let cli = cli::build();
    let matches = match cli.try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            let code = e.exit_code() as i32;
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
