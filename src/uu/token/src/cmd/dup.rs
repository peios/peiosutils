// `token duplicate` — produce a new token with possibly different
// access mask / type / impersonation level. Prints the resulting fd
// (raw — the lifetime of the fd is bounded by this process exiting
// so this is mostly useful inside a longer-lived stub or in tests).

use crate::cmd;
use crate::error::{Error, Result};
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::target::TargetSpec;
use peios::token::{ImpersonationLevel, TokenAccess, TokenType};
use serde_json::json;
use std::os::fd::{AsRawFd, IntoRawFd};

const KACS_TOKEN_DUPLICATE: u32 = TokenAccess::DUPLICATE.bits();

pub fn run(
    matches: &clap::ArgMatches,
    target: TargetSpec,
    mode: OutputMode,
) -> Result<()> {
    let token_type = match matches.get_one::<String>("type").map(String::as_str) {
        Some("primary") | None => TokenType::Primary,
        Some("impersonation") | Some("imp") => TokenType::Impersonation,
        Some(other) => {
            return Err(Error::Usage(format!(
                "--type: expected primary|impersonation, got `{other}`"
            )));
        }
    };
    let level = match matches.get_one::<String>("level").map(String::as_str) {
        Some("anonymous") | Some("anon") => ImpersonationLevel::Anonymous,
        Some("identification") | Some("id") => ImpersonationLevel::Identification,
        Some("impersonation") | Some("imp") | None => ImpersonationLevel::Impersonation,
        Some("delegation") | Some("del") => ImpersonationLevel::Delegation,
        Some(other) => {
            return Err(Error::Usage(format!(
                "--level: expected anonymous|identification|impersonation|delegation, got `{other}`"
            )));
        }
    };
    let access = *matches.get_one::<u32>("access").unwrap_or(&0);

    let tok = target.open(KACS_TOKEN_DUPLICATE)?;
    let dup = tok.duplicate(TokenAccess::from_bits_retain(access), token_type, level)?;

    let mut lines = Lines::new();
    lines.section("duplicate");
    lines.kv("token_type", format!("{token_type:?}"));
    lines.kv("impersonation_level", format!("{level:?}"));
    lines.kv("access_mask", format!("0x{access:x}"));
    lines.kv("result_fd", dup.as_raw_fd().to_string());

    // Leak the fd to the caller — closing on drop would defeat the point
    // of a fd-producing subcommand. `into_raw_fd()` consumes the Token.
    let raw_fd = dup.into_raw_fd();
    let out = json!({
        "token_type": format!("{token_type:?}"),
        "impersonation_level": format!("{level:?}"),
        "access_mask": access,
        "result_fd": raw_fd,
    });
    cmd::emit(CmdOutput { human: lines, json: out }, mode)
}
