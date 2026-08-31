// spell-checker:ignore (libs) mkirf initramfs cpio uucore uumain
//! mkirf ~ (peiosutils) — compile an initramfs source tree into a deterministic compressed cpio.
//!
//! `mkirf <src-dir> <out-file>` walks `<src-dir>`, whose contents map 1:1 onto
//! `/` inside the initramfs, and writes a compressed newc cpio archive (zstd
//! by default, `--compress gzip` for a kernel without zstd support) to
//! `<out-file>`. The output is byte-deterministic: identical input trees
//! produce identical archives. `--watch` keeps the archive current as the
//! source tree is edited like an ordinary directory. See `DESIGN.md`.

use clap::Command;
use uucore::error::{UResult, USimpleError};

pub mod build;
pub mod cli;
pub mod cpio;
pub mod error;
pub mod hooks;
pub mod walk;
pub mod watch;

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match cli::build().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            // clap prints help/version (exit 0) and usage errors (exit 2)
            // itself; relay its exit code to the multi-call runtime.
            let code = e.exit_code();
            e.print().ok();
            return if code == 0 {
                Ok(())
            } else {
                Err(USimpleError::new(code, ""))
            };
        }
    };

    let cfg = cli::Config::from_matches(&matches);
    let excludes = walk::Excludes::compile(&cfg.excludes)
        .map_err(|e| USimpleError::new(2, format!("--exclude: {e}")))?;
    let result = if cfg.watch {
        watch::watch(&cfg.src, &cfg.out, cfg.debounce_secs, &excludes, cfg.compress)
    } else {
        build::run(&cfg.src, &cfg.out, &excludes, cfg.compress)
    };
    result.map_err(|e| USimpleError::new(e.exit_code(), e.to_string()))
}

pub fn uu_app() -> Command {
    cli::build()
}
