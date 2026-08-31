// spell-checker:ignore (libs) mkirf initramfs cpio debounce zstd
//! Command-line surface.
//!
//!   mkirf [--watch] [--debounce <secs>] [--compress <algo>] <src-dir> <out-file>
//!
//! `<src-dir>` maps 1:1 onto `/` inside the initramfs; `<out-file>` is the
//! compressed newc cpio archive. `--watch` stays resident and rebuilds
//! on change (a foreground loop a service manager supervises).

use std::path::PathBuf;

use clap::{Arg, ArgAction, Command};

/// Default debounce window for `--watch`, in seconds.
const DEFAULT_DEBOUNCE_SECS: u64 = 5;

/// The main archive's compressor. Both are kernel decompressor formats
/// (`CONFIG_RD_ZSTD` / `CONFIG_RD_GZIP`); the early region is never
/// compressed either way. zstd is the default on measurement: on a real
/// image it compresses ~9× faster than gzip to a slightly smaller
/// archive, and the kernel decompresses it ~6× faster at every boot.
/// gzip remains for a kernel built without zstd support.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Compress {
    Zstd,
    Gzip,
}

pub fn build() -> Command {
    Command::new("mkirf")
        .about("compile an initramfs source tree into a deterministic compressed cpio")
        .arg(
            Arg::new("watch")
                .long("watch")
                .action(ArgAction::SetTrue)
                .help("stay resident and rebuild on every change to <src-dir>"),
        )
        .arg(
            Arg::new("debounce")
                .long("debounce")
                .value_name("SECS")
                .value_parser(clap::value_parser!(u64))
                .default_value("5")
                .help("with --watch, settle time before a rebuild"),
        )
        .arg(
            Arg::new("exclude")
                .long("exclude")
                .value_name("GLOB")
                .action(ArgAction::Append)
                .help(
                    "exclude paths (relative to <src-dir>) matching GLOB; repeatable. \
                     `*`/`?` stay within a path segment, `**` crosses separators; a \
                     matched directory is pruned with its subtree",
                ),
        )
        .arg(
            Arg::new("compress")
                .long("compress")
                .value_name("ALGO")
                .value_parser(["zstd", "gzip"])
                .default_value("zstd")
                .help("main-archive compressor; the kernel must be built to decompress it"),
        )
        .arg(
            Arg::new("src")
                .value_name("SRC-DIR")
                .required(true)
                .value_parser(clap::value_parser!(PathBuf))
                .help("source tree; its contents map onto / in the initramfs"),
        )
        .arg(
            Arg::new("out")
                .value_name("OUT-FILE")
                .required(true)
                .value_parser(clap::value_parser!(PathBuf))
                .help("output compressed cpio archive"),
        )
}

/// The validated invocation.
pub struct Config {
    pub watch: bool,
    pub debounce_secs: u64,
    pub compress: Compress,
    pub src: PathBuf,
    pub out: PathBuf,
    pub excludes: Vec<String>,
}

impl Config {
    pub fn from_matches(m: &clap::ArgMatches) -> Self {
        Self {
            watch: m.get_flag("watch"),
            debounce_secs: m
                .get_one::<u64>("debounce")
                .copied()
                .unwrap_or(DEFAULT_DEBOUNCE_SECS),
            compress: match m.get_one::<String>("compress").map(String::as_str) {
                Some("gzip") => Compress::Gzip,
                _ => Compress::Zstd,
            },
            src: m.get_one::<PathBuf>("src").cloned().unwrap_or_default(),
            out: m.get_one::<PathBuf>("out").cloned().unwrap_or_default(),
            excludes: m
                .get_many::<String>("exclude")
                .map(|v| v.cloned().collect())
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_clap_config() {
        build().debug_assert();
    }

    #[test]
    fn parses_positionals_with_defaults() {
        let m = build()
            .try_get_matches_from(["mkirf", "/boot/initramfs", "/system/boot/initramfs.cpio.gz"])
            .unwrap();
        let cfg = Config::from_matches(&m);
        assert!(!cfg.watch);
        assert_eq!(cfg.debounce_secs, DEFAULT_DEBOUNCE_SECS);
        assert_eq!(cfg.compress, Compress::Zstd);
        assert_eq!(cfg.src, PathBuf::from("/boot/initramfs"));
        assert_eq!(cfg.out, PathBuf::from("/system/boot/initramfs.cpio.gz"));
    }

    #[test]
    fn parses_compress() {
        let m = build()
            .try_get_matches_from(["mkirf", "--compress", "gzip", "s", "o"])
            .unwrap();
        assert_eq!(Config::from_matches(&m).compress, Compress::Gzip);
        assert!(
            build()
                .try_get_matches_from(["mkirf", "--compress", "lz4", "s", "o"])
                .is_err()
        );
    }

    #[test]
    fn parses_watch_and_debounce() {
        let m = build()
            .try_get_matches_from(["mkirf", "--watch", "--debounce", "2", "src", "out"])
            .unwrap();
        let cfg = Config::from_matches(&m);
        assert!(cfg.watch);
        assert_eq!(cfg.debounce_secs, 2);
    }

    #[test]
    fn parses_repeated_excludes() {
        let m = build()
            .try_get_matches_from([
                "mkirf",
                "--exclude",
                "var/lib/peipkg",
                "--exclude",
                "conf/peipkg/**",
                "s",
                "o",
            ])
            .unwrap();
        let cfg = Config::from_matches(&m);
        assert_eq!(cfg.excludes, ["var/lib/peipkg", "conf/peipkg/**"]);
    }

    #[test]
    fn requires_both_positionals() {
        assert!(build().try_get_matches_from(["mkirf", "only-one"]).is_err());
    }

    #[test]
    fn rejects_non_numeric_debounce() {
        assert!(
            build()
                .try_get_matches_from(["mkirf", "--debounce", "soon", "s", "o"])
                .is_err()
        );
    }
}
