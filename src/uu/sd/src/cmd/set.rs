// `sd set <path> <SDDL>` or `sd set <path> --binary <FILE|->`.
//
// SDDL form: parse via libp_sd::sddl::parse, build, infer SecurityInfo
// from which sections (O:/G:/D:/S:) appeared.
// Binary form: read raw self-relative bytes, pass through.

use crate::cmd::{OutputMode, parse_output_mode, parse_path_target};
use crate::error::{Error, Result};
use clap::ArgMatches;
use libp_sd::{SecurityDescriptor, SecurityInfo, sddl, set_sd};
use libp_sd::consts::{SE_DACL_PRESENT, SE_SACL_PRESENT};
use serde_json::json;
use std::io::Read;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let target = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);

    let (bytes, info) = if let Some(path) = matches.get_one::<String>("binary") {
        let bytes = read_bytes(path)?;
        let parsed = SecurityDescriptor::parse(&bytes)
            .map_err(|e| Error::Invalid(format!("input SD did not parse: {e}")))?;
        let info = infer_info(&parsed);
        (bytes, info)
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
        let builder = sddl::parse(&sddl_str)
            .map_err(|e| Error::Invalid(format!("SDDL parse: {e}")))?;
        let bytes = builder
            .build()
            .map_err(|e| Error::Invalid(format!("SDDL build: {e}")))?;
        let parsed = SecurityDescriptor::parse(&bytes)
            .map_err(|e| Error::Invalid(format!("re-parsing built SD: {e}")))?;
        let info = infer_info(&parsed);
        (bytes, info)
    } else {
        return Err(Error::Usage("provide either <SDDL> or --binary".into()));
    };

    let info = match matches.get_one::<String>("components") {
        Some(s) => override_info(s)?,
        None => info,
    };

    if info.bits() == 0 {
        return Err(Error::Usage(
            "no SD components selected; nothing to write".into(),
        ));
    }

    set_sd(&target.as_sd_target(), info, &bytes).map_err(Error::from)?;

    match mode {
        OutputMode::Human => {
            println!(
                "{}: wrote {} bytes (components: 0x{:08x})",
                target.path,
                bytes.len(),
                info.bits()
            );
        }
        OutputMode::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "path": target.path,
                    "bytes": bytes.len(),
                    "components": format!("0x{:08x}", info.bits()),
                }))
                .unwrap()
            );
        }
    }
    Ok(())
}

/// Infer which `SecurityInfo` bits to touch from which components the SD
/// actually contains.
fn infer_info(sd: &SecurityDescriptor<'_>) -> SecurityInfo {
    let mut info = SecurityInfo::none();
    if sd.owner_off != 0 {
        info = info.with_owner();
    }
    if sd.group_off != 0 {
        info = info.with_group();
    }
    if sd.control & SE_DACL_PRESENT != 0 {
        info = info.with_dacl();
    }
    if sd.control & SE_SACL_PRESENT != 0 {
        info = info.with_sacl();
    }
    info
}

fn override_info(s: &str) -> Result<SecurityInfo> {
    let mut info = SecurityInfo::none();
    for piece in s.split(',') {
        info = match piece.trim().to_ascii_lowercase().as_str() {
            "owner" => info.with_owner(),
            "group" => info.with_group(),
            "dacl" => info.with_dacl(),
            "sacl" => info.with_sacl(),
            "label" | "il" => info.with_label(),
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
