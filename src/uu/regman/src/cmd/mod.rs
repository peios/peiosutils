// Subcommand dispatch.

use clap::ArgMatches;

use crate::error::Result;

pub mod fmt;
pub mod index;
pub mod lint;
pub mod show;

pub fn dispatch(matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("index", sm)) => index::run(sm),
        Some(("fmt", sm)) => fmt::run(sm),
        Some(("lint", sm)) => lint::run(sm),
        // No subcommand ⇒ a `regman <path> [value]` lookup.
        _ => show::run(matches),
    }
}
