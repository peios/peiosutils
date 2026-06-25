// reg ~ (peiosutils) — entry point.
//
// CLI for the live LCS registry (kernel-mediated, layered, Windows-registry-
// shaped configuration store). See `docs/reg-spec.md` for the full design.

use clap::Command;
use uucore::error::{UResult, USimpleError};

pub mod addr;
pub mod cli;
pub mod cmd;
pub mod error;
pub mod literal;
pub mod render;
pub mod settings;

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let cli = cli::build();
    let matches = match cli.try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            // clap encodes --help/--version as non-error "errors" whose
            // exit_code() is 0. Honour that so `--version` exits 0 and a bad
            // flag exits 1.
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
