// stratafs ~ read-only inspection and resolution diagnostics.

use std::io::Write;

use clap::Command;
use uucore::error::{UResult, USimpleError, set_exit_code};

pub mod cli;
pub mod error;
pub mod inspect;
pub mod model;
pub mod output;

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match cli::build().try_get_matches_from(args) {
        Ok(matches) => matches,
        Err(error) => {
            let code = error.exit_code();
            error.print().ok();
            return if code == 0 {
                Ok(())
            } else {
                Err(USimpleError::new(2, ""))
            };
        }
    };
    if let Err(error) = dispatch(&matches) {
        return Err(USimpleError::new(error.exit_code(), error.to_string()));
    }
    Ok(())
}

fn dispatch(matches: &clap::ArgMatches) -> error::Result<()> {
    let entries = model::read_mountinfo()?;
    let mounts = model::strata_mounts(&entries)?;
    match matches.subcommand() {
        Some(("list", args)) => {
            let json = args.get_flag(cli::opt::JSON);
            let reports = inspect::list_reports(
                &mounts,
                args.get_one::<std::ffi::OsString>(cli::opt::MOUNT)
                    .map(std::ffi::OsString::as_os_str),
                json,
            )?;
            if json {
                output::json(&reports)
            } else {
                output::mounts(&reports);
                Ok(())
            }
        }
        Some(("resolve", args)) => {
            let json = args.get_flag(cli::opt::JSON);
            let path = args
                .get_one::<std::ffi::OsString>(cli::opt::PATH)
                .expect("required by clap");
            let report = inspect::resolve_report(&entries, &mounts, path, json)?;
            if json {
                output::json(&report)
            } else {
                output::resolve(&report);
                Ok(())
            }
        }
        Some(("origin", args)) => {
            let path = args
                .get_one::<std::ffi::OsString>(cli::opt::PATH)
                .expect("required by clap");
            let path = model::absolute_lexical(std::path::Path::new(path))?;
            model::find_mount(&mounts, &path)
                .ok_or_else(|| error::Error::NotStratafs(model::display_path(&path)))?;
            let value = inspect::read_origin(&path)?;
            let stdout = std::io::stdout();
            let mut output = stdout.lock();
            output
                .write_all(&value)
                .and_then(|()| {
                    if value.ends_with(b"\n") {
                        Ok(())
                    } else {
                        output.write_all(b"\n")
                    }
                })
                .map_err(|error| error::Error::io("write origin", error))
        }
        Some(("sweep", args)) => {
            let json = args.get_flag(cli::opt::JSON);
            let reports = inspect::sweep_reports(
                &mounts,
                args.get_one::<std::ffi::OsString>(cli::opt::MOUNT)
                    .map(std::ffi::OsString::as_os_str),
                json,
            )?;
            if !reports.is_empty() {
                set_exit_code(1);
            }
            if json {
                output::json(&reports)
            } else {
                output::sweep(&reports);
                Ok(())
            }
        }
        Some(("diff", args)) => {
            let path = args
                .get_one::<std::ffi::OsString>(cli::opt::PATH)
                .expect("required by clap");
            if inspect::diff_path(&mounts, path)? {
                set_exit_code(1);
            }
            Ok(())
        }
        _ => unreachable!("subcommand required by clap"),
    }
}

pub fn uu_app() -> Command {
    cli::build()
}
