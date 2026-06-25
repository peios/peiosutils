// `reg sd <key>` — show or set a key's security descriptor as SDDL.
//
// Reuses the same SDDL codec as the `sd` tool, so output is consistent.

use crate::cmd;
use crate::error::{Error, Result};
use crate::settings::Settings;
use clap::ArgMatches;
use peios::registry::{KeyAccess, OpenFlags, SecInfo};
use peios::security::sddl;
use serde_json::json;

pub fn run(m: &ArgMatches) -> Result<()> {
    let set = Settings::from_matches(m)?;
    let path = cmd::key_path(m)?;
    let target = path.display(set.sep);
    let scope = scope_from_flags(m);
    let setting = m.get_one::<String>("set");

    // Access depends on which components we touch.
    let mut access = KeyAccess::READ_CONTROL;
    if scope.contains(SecInfo::SACL) {
        access |= KeyAccess::ACCESS_SYSTEM_SECURITY;
    }
    if setting.is_some() {
        if scope.intersects(SecInfo::DACL | SecInfo::LABEL) {
            access |= KeyAccess::WRITE_DAC;
        }
        if scope.intersects(SecInfo::OWNER | SecInfo::GROUP) {
            access |= KeyAccess::WRITE_OWNER;
        }
    }
    let key = cmd::open(&path, access, OpenFlags::empty(), &set)?;

    if let Some(sddl_text) = setting {
        let desc = sddl::parse(sddl_text)
            .map_err(|e| Error::from_peios("parse SDDL", sddl_text, e))?;
        key.set_security(scope, &desc, None)
            .map_err(|e| Error::from_peios("set security", &target, e))?;
        cmd::report(
            &set,
            json!({ "key": target, "sd_set": sddl_text }),
            &format!("set security descriptor on {target}"),
        );
        return Ok(());
    }

    let desc = key
        .get_security(scope)
        .map_err(|e| Error::from_peios("get security", &target, e))?;
    let text = sddl::format(desc.as_bytes())
        .map_err(|e| Error::from_peios("format SDDL", &target, e))?;
    if set.json {
        println!("{}", serde_json::to_string_pretty(&json!({ "key": target, "sddl": text })).unwrap_or_default());
    } else {
        println!("{text}");
    }
    Ok(())
}

/// Build the security-info scope from the component flags. Default (no flags):
/// owner + group + DACL (the SACL needs the privileged flag).
fn scope_from_flags(m: &ArgMatches) -> SecInfo {
    let owner = m.get_flag("owner");
    let group = m.get_flag("group");
    let dacl = m.get_flag("dacl");
    let sacl = m.get_flag("sacl");
    if !(owner || group || dacl || sacl) {
        return SecInfo::OWNER | SecInfo::GROUP | SecInfo::DACL;
    }
    let mut s = SecInfo::empty();
    if owner {
        s |= SecInfo::OWNER;
    }
    if group {
        s |= SecInfo::GROUP;
    }
    if dacl {
        s |= SecInfo::DACL;
    }
    if sacl {
        s |= SecInfo::SACL;
    }
    s
}
