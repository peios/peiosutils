// logonse ~ (peiosutils) — Peios logon-session lifecycle and PSB.
//
// Surface (see peios/token-design.md):
//   logonse list                   — enumerate sessions (walks /proc)
//   logonse show <id>              — pids in a session
//   logonse create <SPEC>          — privileged; prints new session id
//   logonse destroy <id>           — destroy empty session
//   logonse psb --pid N --mitigations <mask>
//
// Design caveat: there is no session-enumeration syscall in KACS.
// `list`/`show` walk `/proc` and probe each task's token, which is
// best-effort (races with process exit, can't see kernel-private
// sessions). Acceptable for a debug tool; document and move on.

use clap::{Arg, ArgAction, Command};
use libp_token::{Token, create_session, destroy_empty_session, set_psb};
use libp_sys as sys;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use uucore::error::{UResult, USimpleError};

const SYS_PIDFD_OPEN: i64 = 434;
const KACS_TOKEN_QUERY: u32 = 0x0008;

#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match build_cli().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            let code = e.exit_code() as i32;
            e.print().ok();
            return if code == 0 {
                Ok(())
            } else {
                Err(USimpleError::new(code, ""))
            };
        }
    };
    let (name, sub) = matches
        .subcommand()
        .ok_or_else(|| USimpleError::new(1, "logonse: subcommand required (list|show|create|destroy|psb)"))?;
    let json_mode = sub.get_flag("json");
    let res = match name {
        "list" => cmd_list(json_mode),
        "show" => cmd_show(sub, json_mode),
        "create" => cmd_create(sub, json_mode),
        "destroy" => cmd_destroy(sub, json_mode),
        "psb" => cmd_psb(sub, json_mode),
        other => Err(format!("unknown subcommand: {other}")),
    };
    res.map_err(|m| USimpleError::new(1, m))
}

pub fn uu_app() -> Command {
    build_cli()
}

fn build_cli() -> Command {
    Command::new("logonse")
        .version(uucore::crate_version!())
        .about("Manage Peios logon sessions and Process Security Blocks")
        .subcommand_required(true)
        .subcommand(
            Command::new("list")
                .about("Enumerate active logon sessions (walks /proc, best effort)")
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("show")
                .about("Show the pids in a session")
                .arg(
                    Arg::new("session-id")
                        .required(true)
                        .help("Session id (u32)")
                        .value_parser(clap::value_parser!(u32)),
                )
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("create")
                .about("Create a new logon session from a binary spec (privileged)")
                .arg(
                    Arg::new("spec")
                        .value_name("SPEC")
                        .required(true)
                        .help("Path to spec bytes (or `-` for stdin)"),
                )
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("destroy")
                .about("Destroy an empty session")
                .arg(
                    Arg::new("session-id")
                        .required(true)
                        .help("Session id (u64)")
                        .value_parser(clap::value_parser!(u64)),
                )
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("psb")
                .about("Set the Process Security Block mitigation flags on a process")
                .arg(
                    Arg::new("pid")
                        .long("pid")
                        .required(true)
                        .value_name("PID")
                        .value_parser(clap::value_parser!(i32)),
                )
                .arg(
                    Arg::new("mitigations")
                        .long("mitigations")
                        .required(true)
                        .value_name("MASK")
                        .help("Mitigation bitmask (hex or decimal)"),
                )
                .arg(json_flag()),
        )
}

fn json_flag() -> Arg {
    Arg::new("json")
        .long("json")
        .help("Emit JSON instead of human-readable output")
        .action(ArgAction::SetTrue)
}

// ---------------------------------------------------------------------------
// list / show — best-effort /proc walks.
// ---------------------------------------------------------------------------

fn cmd_list(json_mode: bool) -> Result<(), String> {
    let groups = enumerate_sessions()?;
    if json_mode {
        let arr: Vec<_> = groups
            .iter()
            .map(|(sid, pids)| json!({ "session_id": sid, "pids": pids }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap());
        return Ok(());
    }
    if groups.is_empty() {
        println!("(no readable sessions)");
        return Ok(());
    }
    for (sid, pids) in &groups {
        println!("session {sid}  pids: {pids:?}");
    }
    Ok(())
}

fn cmd_show(sub: &clap::ArgMatches, json_mode: bool) -> Result<(), String> {
    let want: u32 = *sub.get_one::<u32>("session-id").unwrap();
    let groups = enumerate_sessions()?;
    let pids = groups.get(&(want as u64)).cloned().unwrap_or_default();
    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "session_id": want, "pids": pids })).unwrap()
        );
        return Ok(());
    }
    println!("session {want}");
    println!("  pids: {pids:?}");
    Ok(())
}

fn enumerate_sessions() -> Result<BTreeMap<u64, Vec<i32>>, String> {
    let mut out: BTreeMap<u64, Vec<i32>> = BTreeMap::new();
    let dir = fs::read_dir("/proc").map_err(|e| format!("read /proc: {e}"))?;
    for entry in dir.flatten() {
        let name = entry.file_name();
        let Some(s) = name.to_str() else { continue };
        let Ok(pid) = s.parse::<i32>() else { continue };
        if let Some(sid) = session_id_for_pid(pid) {
            out.entry(sid).or_default().push(pid);
        }
    }
    Ok(out)
}

fn session_id_for_pid(pid: i32) -> Option<u64> {
    let pidfd = unsafe { sys::syscall2(SYS_PIDFD_OPEN, pid as u64, 0) };
    if pidfd < 0 {
        return None;
    }
    let pidfd = pidfd as i32;
    let tok = Token::open_process(pidfd, KACS_TOKEN_QUERY).ok();
    unsafe {
        let _ = sys::close(pidfd);
    }
    let tok = tok?;
    tok.session_id().ok().map(|v| v as u64)
}

// ---------------------------------------------------------------------------
// create / destroy.
// ---------------------------------------------------------------------------

fn cmd_create(sub: &clap::ArgMatches, json_mode: bool) -> Result<(), String> {
    let path = sub.get_one::<String>("spec").unwrap();
    let spec = read_spec_input(path)?;
    let session_id = create_session(&spec).map_err(|e| format!("create_session: {e}"))?;
    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "session_id": session_id,
                "spec_bytes": spec.len(),
            }))
            .unwrap()
        );
    } else {
        println!("created session {session_id} (spec {} bytes)", spec.len());
    }
    Ok(())
}

fn cmd_destroy(sub: &clap::ArgMatches, json_mode: bool) -> Result<(), String> {
    let id: u64 = *sub.get_one::<u64>("session-id").unwrap();
    destroy_empty_session(id).map_err(|e| format!("destroy_empty_session: {e}"))?;
    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "destroyed_session_id": id })).unwrap()
        );
    } else {
        println!("destroyed session {id}");
    }
    Ok(())
}

fn read_spec_input(path: &str) -> Result<Vec<u8>, String> {
    if path == "-" {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| format!("reading stdin: {e}"))?;
        return Ok(buf);
    }
    fs::read(Path::new(path)).map_err(|e| format!("reading {path}: {e}"))
}

// ---------------------------------------------------------------------------
// psb.
// ---------------------------------------------------------------------------

fn cmd_psb(sub: &clap::ArgMatches, json_mode: bool) -> Result<(), String> {
    let pid: i32 = *sub.get_one::<i32>("pid").unwrap();
    let mitigations_str: &String = sub.get_one::<String>("mitigations").unwrap();
    let mitigations = parse_mask(mitigations_str)?;

    let pidfd = unsafe { sys::syscall2(SYS_PIDFD_OPEN, pid as u64, 0) };
    if pidfd < 0 {
        return Err(format!(
            "pidfd_open(pid={pid}): -E{}",
            -pidfd as i32
        ));
    }
    let pidfd = pidfd as i32;
    let r = set_psb(pidfd, mitigations);
    unsafe {
        let _ = sys::close(pidfd);
    }
    r.map_err(|e| format!("set_psb: {e}"))?;

    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "pid": pid,
                "mitigations": format!("0x{mitigations:x}"),
            }))
            .unwrap()
        );
    } else {
        println!("psb pid={pid} mitigations=0x{mitigations:x}");
    }
    Ok(())
}

fn parse_mask(s: &str) -> Result<u32, String> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).map_err(|e| format!("bad mask `{s}`: {e}"))
    } else {
        t.parse::<u32>().map_err(|e| format!("bad mask `{s}`: {e}"))
    }
}
