// Subcommand dispatch.

use clap::ArgMatches;

use crate::error::Result;

pub mod lint;
pub mod show;

pub fn dispatch(matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("lint", sm)) => lint::run(sm),
        // No subcommand ⇒ an `evman <name>` lookup.
        _ => show::run(matches),
    }
}
