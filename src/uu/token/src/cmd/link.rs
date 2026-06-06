// `token link` and `token linked` — manage the elevation-pair link
// between two tokens (UAC-style).

use crate::cmd;
use crate::error::{Error, Result};
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::target::TargetSpec;
use libp_token::Token;
use libp_token::uapi::KACS_TOKEN_QUERY;
use serde_json::json;

pub fn link(matches: &clap::ArgMatches, mode: OutputMode) -> Result<()> {
    let elevated_fd = *matches
        .get_one::<i32>("elevated")
        .ok_or_else(|| Error::Usage("link: --elevated <fd> required".into()))?;
    let filtered_fd = *matches
        .get_one::<i32>("filtered")
        .ok_or_else(|| Error::Usage("link: --filtered <fd> required".into()))?;
    let session_id = *matches
        .get_one::<u64>("session")
        .ok_or_else(|| Error::Usage("link: --session <id> required".into()))?;

    // The caller hands us raw fds. We wrap them in Tokens just long enough
    // to call the ioctl; both Tokens close on drop, which is what we want
    // (the link is persisted kernel-side, not via the fds).
    let elevated = unsafe { Token::from_raw_fd(elevated_fd) };
    let filtered = unsafe { Token::from_raw_fd(filtered_fd) };
    let result = elevated.link_tokens(&filtered, session_id);

    // Reclaim the fds so we don't double-close: the caller is the fd owner.
    let _ = elevated.into_raw_fd();
    let _ = filtered.into_raw_fd();
    result?;

    let mut lines = Lines::new();
    lines.section("link");
    lines.kv("elevated_fd", elevated_fd.to_string());
    lines.kv("filtered_fd", filtered_fd.to_string());
    lines.kv("session_id", session_id.to_string());
    let out = json!({
        "elevated_fd": elevated_fd,
        "filtered_fd": filtered_fd,
        "session_id": session_id,
    });
    cmd::emit(CmdOutput { human: lines, json: out }, mode)
}

pub fn linked(target: TargetSpec, mode: OutputMode) -> Result<()> {
    let tok = target.open(KACS_TOKEN_QUERY)?;
    let linked = tok.linked_token()?;
    let fd = linked.as_raw_fd();
    let raw_fd = linked.into_raw_fd();
    let mut lines = Lines::new();
    lines.section("linked");
    lines.kv("linked_fd", fd.to_string());
    let out = json!({ "linked_fd": raw_fd });
    cmd::emit(CmdOutput { human: lines, json: out }, mode)
}
