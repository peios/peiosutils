// Command-line surface.
//
//   evman <name>                   explain an event type, a field, or a prefix
//   evman -k <term>...             search names and summaries
//   evman lint <file>...           check fragments against PGSS §6.10
//
// There is no `fmt`: an evman anchor is the name itself, so there is nothing
// folded to bake. There is no `index`: it is an accelerator for a corpus far
// larger than the event catalogue.

use clap::{Arg, Command};

pub fn build() -> Command {
    Command::new("evman")
        .about("the Peios event manual")
        .args_conflicts_with_subcommands(true)
        .subcommand_negates_reqs(true)
        .arg(
            Arg::new("name")
                .help("event type, field path, or a prefix of either (e.g. kacs.audit)")
                .index(1),
        )
        .arg(
            Arg::new("apropos")
                .short('k')
                .long("apropos")
                .num_args(1..)
                .value_name("TERM")
                .conflicts_with("name")
                .help("search event type and field names and summaries for TERM(s)"),
        )
        .subcommand(
            Command::new("lint")
                .about("check fragments against the catalogue rules (PGSS §6.10)")
                .arg(Arg::new("files").num_args(1..).required(true)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_clap_config() {
        build().debug_assert();
    }

    #[test]
    fn parses_name() {
        let m = build()
            .try_get_matches_from(["evman", "kacs.audit.access.checked"])
            .unwrap();
        assert_eq!(m.get_one::<String>("name").unwrap(), "kacs.audit.access.checked");
    }

    #[test]
    fn parses_apropos_terms() {
        let m = build().try_get_matches_from(["evman", "-k", "token", "sid"]).unwrap();
        let terms: Vec<_> = m.get_many::<String>("apropos").unwrap().collect();
        assert_eq!(terms, ["token", "sid"]);
    }

    #[test]
    fn parses_lint_files() {
        let m = build()
            .try_get_matches_from(["evman", "lint", "a.evman", "b.evman"])
            .unwrap();
        let (name, sm) = m.subcommand().unwrap();
        assert_eq!(name, "lint");
        assert_eq!(sm.get_many::<String>("files").unwrap().count(), 2);
    }

    #[test]
    fn lint_needs_a_file() {
        assert!(build().try_get_matches_from(["evman", "lint"]).is_err());
    }
}
