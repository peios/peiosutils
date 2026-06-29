// feat ~ (peiosutils) — entry point.
//
// A *feature* is the most basic mechanism for "I need to be more than just
// install files." It is a directory under `/usr/libexec/peios/features.d/<name>/`
// holding up to four lifecycle scripts — `install.sh`, `enable.sh`,
// `disable.sh`, `uninstall.sh` — any of which may be absent (a missing script
// is a no-op for that phase). It is the deliberately low-level, imperative layer
// **above packages** and **below roles/applets**: packages are pure declarative
// file delivery; roles/applets are declarative and constrained; features are the
// raw shell escape hatch for first-party (or exceptionally-empowered third
// party) needs. Their awkwardness is the point — it keeps broad shell power rare.
//
// feat does not escalate. It runs each script as a normal child, so the script
// inherits the *caller's* token; KACS governs what it may do. A user with an
// admin-grade token gets admin-grade scripts; a weak token gets EPERM. feat
// grants nothing — it is a runner, not a broker.
//
// What is installed/enabled is *state*, and like all peios state it lives in the
// registry: `Machine\System\Features\<name>` -> `Installed` / `Enabled`. The
// feature directory is read-only definition; the registry is the source of truth.

use clap::Command;
use uucore::error::{UResult, USimpleError};

pub mod cli;
pub mod cmd;
pub mod error;
pub mod feature;
pub mod registry;

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match cli::build().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            // clap encodes --help/--version as exit-0 "errors"; honour that.
            let code = e.exit_code();
            e.print().ok();
            return if code == 0 {
                Ok(())
            } else {
                Err(USimpleError::new(code, ""))
            };
        }
    };
    match cmd::dispatch(&matches) {
        Ok(()) => Ok(()),
        Err(err) => Err(USimpleError::new(err.exit_code(), err.to_string())),
    }
}

pub fn uu_app() -> Command {
    cli::build()
}
