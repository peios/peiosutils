// `sd set <path> <SDDL>` or `sd set <path> --binary <FILE|->`.
//
// SDDL form: parse via `peios::security::sddl::parse`, take its bytes, infer
// SecInfo from which sections (O:/G:/D:/S:) appeared.
// Binary form: read raw self-relative bytes, pass through.

use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use clap::ArgMatches;
use peios::file::{SecInfo, set_sd};
use peios::security::{Control, SdView, SecurityDescriptor, sddl};
use serde_json::json;
use std::io::Read;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let target = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);

    let (sd, info) = if let Some(path) = matches.get_one::<String>("binary") {
        let raw = read_bytes(path)?;
        let view = SdView::parse(&raw)
            .map_err(|e| Error::Invalid(format!("input SD did not parse: {e}")))?;
        let info = infer_info(&view);
        // `set_sd` takes an owned `SecurityDescriptor`, and the crate exposes no
        // public `from_bytes`; round-trip the raw bytes through SDDL to obtain one.
        let sd = sd_from_bytes(&raw)?;
        (sd, info)
    } else if let Some(s) = matches.get_one::<String>("sddl") {
        let sddl_str = if s == "-" {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| Error::Invalid(format!("reading SDDL stdin: {e}")))?;
            buf
        } else {
            s.clone()
        };
        let parsed = sddl::parse(&sddl_str)
            .map_err(|e| Error::Invalid(format!("SDDL parse: {e}")))?;
        let view = SdView::parse(parsed.as_bytes())
            .map_err(|e| Error::Invalid(format!("re-parsing built SD: {e}")))?;
        let info = infer_info(&view);
        (parsed, info)
    } else {
        return Err(Error::Usage("provide either <SDDL> or --binary".into()));
    };

    let info = match matches.get_one::<String>("components") {
        Some(s) => override_info(s)?,
        None => info,
    };

    if info.is_empty() {
        return Err(Error::Usage(
            "no SD components selected; nothing to write".into(),
        ));
    }

    let bytes_len = sd.as_bytes().len();
    set_sd(
        target.dirfd(),
        target.as_path(),
        info,
        &sd,
        target.at_flags(),
    )
    .map_err(Error::from)?;

    match mode {
        OutputMode::Human => {
            println!(
                "{}: wrote {} bytes (components: 0x{:08x})",
                target.path,
                bytes_len,
                info.bits()
            );
        }
        OutputMode::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "path": target.path,
                    "bytes": bytes_len,
                    "components": format!("0x{:08x}", info.bits()),
                }))
                .unwrap()
            );
        }
    }
    Ok(())
}

/// Wrap raw self-relative SD bytes into an owned `SecurityDescriptor`. The
/// `peios` crate exposes no public `from_bytes`, so round-trip through SDDL,
/// which is lossless for a valid descriptor.
fn sd_from_bytes(bytes: &[u8]) -> Result<SecurityDescriptor> {
    let text = sddl::format(bytes)
        .map_err(|e| Error::Invalid(format!("rendering SD to SDDL: {e}")))?;
    sddl::parse(&text).map_err(|e| Error::Invalid(format!("re-encoding SD: {e}")))
}

/// Infer which `SecInfo` bits to touch from which components the SD contains.
fn infer_info(sd: &SdView<'_>) -> SecInfo {
    let mut info = SecInfo::empty();
    if sd.owner().is_some() {
        info |= SecInfo::OWNER;
    }
    if sd.group().is_some() {
        info |= SecInfo::GROUP;
    }
    let control = sd.control();
    if control.contains(Control::DACL_PRESENT) {
        info |= SecInfo::DACL;
    }
    if control.contains(Control::SACL_PRESENT) {
        info |= SecInfo::SACL;
    }
    info
}

fn override_info(s: &str) -> Result<SecInfo> {
    let mut info = SecInfo::empty();
    for piece in s.split(',') {
        info |= match piece.trim().to_ascii_lowercase().as_str() {
            "owner" => SecInfo::OWNER,
            "group" => SecInfo::GROUP,
            "dacl" => SecInfo::DACL,
            "sacl" => SecInfo::SACL,
            "label" | "il" => SecInfo::LABEL,
            other => {
                return Err(Error::Usage(format!(
                    "unknown component `{other}` (use owner,group,dacl,sacl,label)"
                )));
            }
        };
    }
    Ok(info)
}

fn read_bytes(path: &str) -> Result<Vec<u8>> {
    if path == "-" {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| Error::Invalid(format!("reading binary stdin: {e}")))?;
        return Ok(buf);
    }
    std::fs::read(path).map_err(|e| Error::NotFound(format!("{path}: {e}")))
}
