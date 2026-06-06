// `regman index [--watch]` and `regman index clear`.

use clap::ArgMatches;

use crate::corpus;
use crate::error::Result;
use crate::index;
use crate::watch;

pub fn run(matches: &ArgMatches) -> Result<()> {
    if matches.subcommand_matches("clear").is_some() {
        return index::clear(&corpus::index_path());
    }

    let dir = corpus::dir();
    let index_path = corpus::index_path();
    index::build(&dir, &index_path)?;

    if matches.get_flag("watch") {
        // Resident: rebuild whenever the corpus changes. peinit supervises this
        // (like `mkirf --watch`); see design §7.4.
        watch::run(&dir, &index_path)?;
    }
    Ok(())
}
