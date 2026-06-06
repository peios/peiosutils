// Command-line surface (design §8).
//
//   regman <path> [value]          explain a key or value
//   regman index [--watch]         build / maintain the lookup cache
//   regman index clear             remove the cache
//   regman fmt  <file>...          bake folded anchors from canonical
//   regman lint <file>...          verify fragment structure and anchors

use clap::{Arg, ArgAction, Command};

pub fn build() -> Command {
    Command::new("regman")
        .about("the Peios registry manual")
        .args_conflicts_with_subcommands(true)
        .subcommand_negates_reqs(true)
        .arg(
            Arg::new("path")
                .help("registry key path (e.g. Machine\\System\\KMES)")
                .index(1),
        )
        .arg(
            Arg::new("value")
                .help("value name within the key")
                .index(2),
        )
        .arg(
            Arg::new("apropos")
                .short('k')
                .long("apropos")
                .num_args(1..)
                .value_name("TERM")
                .conflicts_with_all(["path", "value"])
                .help("search key/value names and summaries for TERM(s)"),
        )
        .subcommand(
            Command::new("index")
                .about("build or maintain the lookup cache")
                .arg(
                    Arg::new("watch")
                        .long("watch")
                        .action(ArgAction::SetTrue)
                        .help("stay resident and rebuild the cache on change"),
                )
                .subcommand(Command::new("clear").about("remove the lookup cache")),
        )
        .subcommand(
            Command::new("fmt")
                .about("bake folded anchors from canonical fields")
                .arg(Arg::new("files").num_args(1..).required(true)),
        )
        .subcommand(
            Command::new("lint")
                .about("verify fragment structure and anchors")
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
    fn parses_path_and_value() {
        let m = build()
            .try_get_matches_from(["regman", "Machine\\System\\KMES", "BufferCapacity"])
            .unwrap();
        assert_eq!(m.get_one::<String>("path").unwrap(), "Machine\\System\\KMES");
        assert_eq!(m.get_one::<String>("value").unwrap(), "BufferCapacity");
    }

    #[test]
    fn parses_index_watch() {
        let m = build().try_get_matches_from(["regman", "index", "--watch"]).unwrap();
        let (name, sm) = m.subcommand().unwrap();
        assert_eq!(name, "index");
        assert!(sm.get_flag("watch"));
    }

    #[test]
    fn parses_index_clear() {
        let m = build().try_get_matches_from(["regman", "index", "clear"]).unwrap();
        let (_, sm) = m.subcommand().unwrap();
        assert_eq!(sm.subcommand().unwrap().0, "clear");
    }
}
