// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! lsblk ~ (peiosutils) — list information about block devices.
//!
//! Layered as a clap-free library core plus a thin CLI ([`cli`] + [`uumain`]),
//! mirroring `pu_mount`. The pipeline is: enumerate the block-device tree from
//! sysfs ([`device`]) → enrich each node with filesystem identity from libblkid
//! ([`uucore::blkid`]) and mount points from `/proc/self/mountinfo`
//! ([`mountinfo`]) → select columns ([`column`]) → render in the requested mode
//! ([`output`]).
//!
//! Column sourcing (see the design notes in the package memory):
//! * **sysfs** — topology + hardware columns. `/sys/block` is unpatched on
//!   peios, so this is the upstream no-udev fallback path verbatim.
//! * **libblkid** — `FSTYPE`/`FSVER`/`UUID`/`LABEL` and the `PART*` columns.
//! * **Security Descriptor** — `OWNER`/`MODE` (`-m`), read off the `/dev` node
//!   via [`uucore::sd_control`] and rendered exactly like peios `ls -l`
//!   (owner SID + `[type][x][+]`; no `GROUP`, no permission bits).
//! * **`/dev/disk/by-*` symlink farm** — `ID`/`ID-LINK`. There is deliberately
//!   no `/run/udev/data` parser; that fast-path waits on peios-udev.

pub mod cli;
pub mod column;
pub mod config;
pub mod device;
pub mod error;
pub mod mountinfo;
pub mod output;
pub mod perms;

use uucore::error::{UResult, USimpleError};

use crate::config::Config;

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match cli::build().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            // clap returns 2 for argument errors; util-linux lsblk uses exit 1
            // for incorrect invocation. Help/version stay 0.
            e.print().ok();
            return if e.use_stderr() {
                Err(USimpleError::new(1, ""))
            } else {
                Ok(())
            };
        }
    };

    let result = (|| {
        let config = Config::from_matches(&matches)?;
        let mut tree = device::enumerate(&config)?;
        config.apply_filters(&mut tree);
        output::render(&tree, &config)
    })();

    result.map_err(|e| USimpleError::new(e.exit_code(), e.to_string()))
}

pub fn uu_app() -> clap::Command {
    cli::build()
}
