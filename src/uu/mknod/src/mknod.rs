// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (ToDO) makedev sysmacros IFBLK IFCHR IFIFO sflag

//! `mknod` for Peios. Creates special files — block and character
//! device nodes and FIFOs.
//!
//! POSIX mode bits, umask, and SELinux contexts do not exist on Peios.
//! The node is created with the Linux `mknod()` namespace syscall — KACS
//! computes its Security Descriptor from the parent directory's
//! inheritable ACEs. The `--sd*` flag group (see `uucore::sd_control`)
//! overrides that, applied post-create with `libp_sd::set_sd`.

use clap::{Arg, Command, value_parser};
use nix::sys::stat::{Mode, SFlag, mknod as nix_mknod};
use std::fs;
use std::path::Path;
use uucore::display::Quotable;
use uucore::error::{UResult, USimpleError, UUsageError};
use uucore::format_usage;
use uucore::fs::makedev;
use uucore::sd_control::{self, CreatorSd};
use uucore::translate;

mod options {
    pub const TYPE: &str = "type";
    pub const MAJOR: &str = "major";
    pub const MINOR: &str = "minor";
    pub const NAME: &str = "name";
}

#[derive(Clone, PartialEq)]
enum FileType {
    Block,
    Character,
    Fifo,
}

impl FileType {
    fn as_sflag(&self) -> SFlag {
        match self {
            Self::Block => SFlag::S_IFBLK,
            Self::Character => SFlag::S_IFCHR,
            Self::Fifo => SFlag::S_IFIFO,
        }
    }
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;

    let file_type = matches.get_one::<FileType>(options::TYPE).unwrap();
    let name = matches
        .get_one::<String>(options::NAME)
        .expect("Missing argument 'NAME'");
    let creator_sd = sd_control::creator_sd_from_matches(&matches)?;

    let dev = match (
        file_type,
        matches.get_one::<u32>(options::MAJOR),
        matches.get_one::<u32>(options::MINOR),
    ) {
        (FileType::Fifo, None, None) => 0,
        (FileType::Fifo, _, _) => {
            return Err(UUsageError::new(
                1,
                translate!("mknod-error-fifo-no-major-minor"),
            ));
        }
        (_, Some(&major), Some(&minor)) => makedev(major as _, minor as _) as u64,
        _ => {
            return Err(UUsageError::new(
                1,
                translate!("mknod-error-special-require-major-minor"),
            ));
        }
    };

    make_node(name, file_type, dev, creator_sd.as_ref())
}

pub fn uu_app() -> Command {
    Command::new("mknod")
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template("mknod"))
        .override_usage(format_usage(&translate!("mknod-usage")))
        .after_help(translate!("mknod-after-help"))
        .about(translate!("mknod-about"))
        .infer_long_args(true)
        .arg(
            Arg::new(options::NAME)
                .value_name("NAME")
                .help(translate!("mknod-help-name"))
                .required(true)
                .value_hint(clap::ValueHint::AnyPath),
        )
        .arg(
            Arg::new(options::TYPE)
                .value_name("TYPE")
                .help(translate!("mknod-help-type"))
                .required(true)
                .value_parser(parse_type),
        )
        .arg(
            Arg::new(options::MAJOR)
                .value_name(options::MAJOR)
                .help(translate!("mknod-help-major"))
                .value_parser(value_parser!(u32)),
        )
        .arg(
            Arg::new(options::MINOR)
                .value_name(options::MINOR)
                .help(translate!("mknod-help-minor"))
                .value_parser(value_parser!(u32)),
        )
        .args(sd_control::args())
}

/// Create one special file and, when `sd` is given, apply it post-create.
///
/// The node is created with the Linux `mknod()` namespace syscall —
/// KACS-native open does not create special nodes — so the descriptor
/// is applied afterward with `libp_sd::set_sd` (create-then-set; see
/// `peios/sd-creation-design.md`). If the descriptor cannot be applied
/// the node is removed, leaving nothing half-secured behind.
fn make_node(name: &str, file_type: &FileType, dev: u64, sd: Option<&CreatorSd>) -> UResult<()> {
    // The raw Linux inode mode is compatibility metadata only on Peios;
    // KACS computes the node's real Security Descriptor.
    nix_mknod(name, file_type.as_sflag(), Mode::from_bits_truncate(0o666), dev as _).map_err(
        |e| {
            USimpleError::new(
                1,
                translate!("mknod-error-cannot-create", "path" => name.quote(), "error" => std::io::Error::from(e)),
            )
        },
    )?;

    if let Some(sd) = sd
        && let Err(e) = sd.apply_to(Path::new(name))
    {
        let _ = fs::remove_file(name);
        return Err(e);
    }

    Ok(())
}

fn parse_type(tpe: &str) -> Result<FileType, String> {
    // Only check the first character, to allow mnemonic usage like
    // 'mknod /dev/rst0 character 18 0'.
    tpe.chars()
        .next()
        .ok_or_else(|| translate!("mknod-error-missing-device-type"))
        .and_then(|first_char| match first_char {
            'b' => Ok(FileType::Block),
            'c' | 'u' => Ok(FileType::Character),
            'p' => Ok(FileType::Fifo),
            _ => Err(translate!("mknod-error-invalid-device-type", "type" => tpe.quote())),
        })
}
