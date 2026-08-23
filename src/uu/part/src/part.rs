// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! part ~ (peiosutils) — manage disk partition tables.
//!
//! `part` fills the slot a Windows user would reach for diskpart to fill, but
//! deliberately not its shape. diskpart is a stateful interactive shell that
//! also formats volumes, assigns drive letters and manages dynamic disks;
//! `part` is one-shot commands over a named device, covering the partition
//! table and nothing else. One-shot is the house style (`sd`, `reg`,
//! `stratafs`) and a select-then-act model is a footgun in scripts, which are
//! this tool's main caller. The rest already has homes: `mke2fs`/`mkfs.vfat`
//! format, `mount` mounts, and Peios has no drive letters.
//!
//! ```text
//!   part list   <disk>
//!   part verify <disk>
//!   part create <disk> --yes
//!   part add    <disk> --size 512M --type esp --name "EFI system partition"
//!   part del    <disk> <n> --yes
//! ```
//!
//! # Layering
//!
//! [`gpt`] is pure: it builds and parses tables as values and never opens a
//! file, so every layout rule is tested against a `Vec<u8>`. [`device`] owns
//! everything that touches hardware — geometry, the safety guards, reading and
//! writing sectors, and asking the kernel to re-read the table. [`label`] is
//! the seam between them, so a second table format could be added without
//! disturbing either side.
//!
//! # Why it refuses things
//!
//! This is the one tool in the tree whose mistakes are unrecoverable, so it
//! reports what it *found* rather than what it failed to find. "No GPT" is
//! ambiguous between a blank disk and an MBR disk holding somebody's data, and
//! those deserve opposite behaviour: the first is the ordinary case for
//! `create`, the second stops until `--force` says otherwise.

pub mod cli;
pub mod device;
pub mod error;
pub mod gpt;
pub mod label;
pub mod types;

use uucore::error::{UResult, USimpleError};

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match cli::build().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            e.print().ok();
            return if e.use_stderr() {
                Err(USimpleError::new(1, ""))
            } else {
                Ok(())
            };
        }
    };

    // The exit code carries the distinction the error type exists to draw:
    // 3 means `part` refused, which is it working correctly, and a caller such
    // as peios-install should stop rather than retry.
    if let Err(e) = cli::dispatch(&matches) {
        return Err(USimpleError::new(e.exit_code(), format!("{e}")));
    }
    Ok(())
}

/// The clap surface, for the multiplexed binary's help and completions.
pub fn uu_app() -> clap::Command {
    cli::build()
}
