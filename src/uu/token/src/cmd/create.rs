// `token create` and `token install` — construct a token from a
// kernel-format spec, and (for install) make it the calling task's
// primary token.
//
// Design note: the design doc described a JSON→msgpack encoding path
// for the spec, but `kacs_create_token` actually consumes a fixed
// 192-byte-header binary spec (see `token_runtime.rs::create_from_spec`,
// PKM_KUNIT_TOKEN_SPEC_VERSION=2). A JSON sugar layer would have to
// mirror that layout byte-for-byte and is deferred; for v1 the tool
// accepts pre-built spec bytes from a file or stdin. This matches the
// "debug tool" framing — anyone driving `token create` directly has
// the kernel header in front of them anyway.

use crate::cmd;
use crate::error::{Error, Result};
use crate::render::{CmdOutput, Lines, OutputMode};
use libp_token::Token;
use serde_json::json;
use std::io::Read;
use std::path::Path;

pub fn create(matches: &clap::ArgMatches, mode: OutputMode) -> Result<()> {
    let spec = read_spec_input(matches)?;
    let tok = Token::create(&spec)?;

    let mut lines = Lines::new();
    lines.section("create");
    lines.kv("spec_bytes", spec.len().to_string());
    lines.kv("result_fd", tok.as_raw_fd().to_string());

    let raw_fd = tok.into_raw_fd();
    let out = json!({
        "spec_bytes": spec.len(),
        "result_fd": raw_fd,
    });
    cmd::emit(CmdOutput { human: lines, json: out }, mode)
}

pub fn install(matches: &clap::ArgMatches, mode: OutputMode) -> Result<()> {
    let spec = read_spec_input(matches)?;
    let tok = Token::create(&spec)?;
    tok.install()?;

    let mut lines = Lines::new();
    lines.section("install");
    lines.kv("spec_bytes", spec.len().to_string());
    lines.kv("status", "installed as primary token");

    let out = json!({
        "spec_bytes": spec.len(),
        "status": "installed",
    });
    cmd::emit(CmdOutput { human: lines, json: out }, mode)
}

fn read_spec_input(matches: &clap::ArgMatches) -> Result<Vec<u8>> {
    let path = matches
        .get_one::<String>("spec")
        .ok_or_else(|| Error::Usage("create/install: missing <SPEC> path argument".into()))?;

    if path == "-" {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| Error::InvalidSpec(format!("reading stdin: {e}")))?;
        return Ok(buf);
    }
    std::fs::read(Path::new(path))
        .map_err(|e| Error::InvalidSpec(format!("reading {path}: {e}")))
}
