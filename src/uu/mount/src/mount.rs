// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! mount ~ (peiosutils) — Peios filesystem mount via the fd-based mount API.
//!
//! Layered as a clap-free library core ([`options`]/[`verb`]/[`request`]/
//! [`flow`]/[`sys`] …) plus a thin CLI ([`cli`] + [`uumain`]), per
//! docs/mount-spec.md §16. The CLI parses into a [`request::Action`]; the
//! executor ([`flow`]) turns a mount request into the new-API syscall sequence.

pub mod blkid;
pub mod cli;
pub mod error;
pub mod flow;
pub mod listing;
pub mod loopdev;
pub mod mountinfo;
pub mod namespace;
pub mod options;
pub mod policy;
pub mod request;
pub mod sys;
pub mod verb;

use uucore::error::{UResult, USimpleError};

use crate::request::Action;

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match cli::build().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            // clap returns 2 for argument errors; util-linux mount uses exit 1
            // for "incorrect invocation" (§10). Help/version stay 0.
            e.print().ok();
            return if e.use_stderr() {
                Err(USimpleError::new(1, ""))
            } else {
                Ok(())
            };
        }
    };

    let result = match request::build(&matches) {
        Ok(Action::List(list)) => listing::run(&list),
        Ok(Action::Mount(req)) => flow::execute(&req),
        Err(e) => Err(e),
    };

    result.map_err(|e| USimpleError::new(e.exit_code(), e.to_string()))
}

pub fn uu_app() -> clap::Command {
    cli::build()
}
