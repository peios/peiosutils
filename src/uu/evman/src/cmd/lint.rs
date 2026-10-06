// `evman lint <file>...` — check fragments against the §6.10 rules.
//
// The files are checked as one catalogue, because the rules that matter most
// span fragments. Each finding prints as `file:line: rule N: message` on
// stdout — the findings are the command's output, as they are the reference
// lint's (pkm/tools/evman.py), whose output this matches line for line. The
// count goes to stderr with exit 3, as regman's lint does.

use std::path::Path;

use clap::ArgMatches;

use crate::error::{Error, Result};
use crate::lint;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let files: Vec<&Path> = matches
        .get_many::<String>("files")
        .map(|f| f.map(Path::new).collect())
        .unwrap_or_default();

    let findings = lint::lint_files(&files)?;
    for finding in &findings {
        println!("{finding}");
    }

    if !findings.is_empty() {
        return Err(Error::Fragment(format!(
            "lint: {} problem(s) found",
            findings.len()
        )));
    }
    Ok(())
}
