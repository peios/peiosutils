// token ~ (peiosutils) — entry point.
//
// See `peios/token-design.md` for the full design.

use clap::Command;
use uucore::error::{UResult, USimpleError};

pub mod cli;
pub mod cmd;
pub mod error;
pub mod payload;
pub mod privs;
pub mod render;
pub mod sid_render;
pub mod target;

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let cli = cli::build();
    let matches = match cli.try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            // clap encodes --help and --version as non-error "errors"
            // whose exit_code() is 0. Honour that so `--version` exits
            // 0 and bad flags exit 1.
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
