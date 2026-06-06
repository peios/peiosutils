// `sd show <path>` — render the file's SD in the requested format.

use crate::cmd::{OutputMode, parse_output_mode, parse_path_target, parse_sid_style};
use crate::error::{Error, Result};
use crate::render;
use clap::ArgMatches;
use libp_sd::{SecurityDescriptor, SecurityInfo, get_sd};
use libp_sd::sddl;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let target = parse_path_target(matches)?;
    let mode = parse_output_mode(matches);
    let style = parse_sid_style(matches)?;
    let want_sddl = matches.get_flag("sddl");
    let want_all = matches.get_flag("all");

    let bytes = get_sd(&target.as_sd_target(), SecurityInfo::all())
        .map_err(Error::from)?;

    if bytes.is_empty() {
        // Kernel has no SD recorded for this object — implicit default applies.
        match mode {
            OutputMode::Human => print!("{}", render::no_sd_human(&target.path)),
            OutputMode::Json => println!(
                "{}",
                serde_json::to_string_pretty(&render::no_sd_json(&target.path)).unwrap()
            ),
        }
        return Ok(());
    }

    let sd = SecurityDescriptor::parse(&bytes)
        .map_err(|e| Error::Invalid(format!("parsing SD bytes: {e}")))?;

    if want_sddl {
        let sddl_str =
            sddl::format(&sd).map_err(|e| Error::Invalid(format!("SDDL render: {e}")))?;
        if matches!(mode, OutputMode::Json) {
            let v = serde_json::json!({ "path": target.path, "sddl": sddl_str });
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        } else {
            println!("{}", target.path);
            println!("  {sddl_str}");
        }
        return Ok(());
    }

    match mode {
        OutputMode::Human => {
            print!("{}", render::sd_human(&target.path, &sd, style));
            if want_all {
                println!("  Raw bytes: {} bytes", bytes.len());
                println!("  Control (raw): 0x{:04x}", sd.control);
            }
        }
        OutputMode::Json => {
            let mut v = render::sd_json(&target.path, &sd);
            if want_all {
                v["raw_bytes_len"] = serde_json::json!(bytes.len());
            }
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
    }
    Ok(())
}
