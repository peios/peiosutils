// logonse ~ (peiosutils) — Peios logon-session lifecycle and PSB.
//
// Surface:
//   logonse list                   — every live session, and its processes
//   logonse show <id>              — one session, and its processes
//   logonse create <SPEC>          — privileged; prints new session id
//   logonse destroy <id>           — destroy empty session
//   logonse end <id>               — sign a session out: authd ends its processes
//   logonse psb --pid N            — read a process's PSB
//   logonse psb --pid N --mitigations <mask> — turn mitigations on
//
// The sessions come from the kernel's own listing (securityfs's
// kacs/sessions), which only Administrators and SYSTEM may read. The
// processes in each come from walking /proc and reading each process's
// token's auth_id, which shows only the processes the caller may inspect.
// Without the listing, the walk is all there is, and the output says so.

use clap::{Arg, ArgAction, Command};
use libauthd_client::logon::{Logon, Refusal};
use peios::process::{Mitigations, Process};
use peios::security::Sid;
use peios::token::{LogonSessionInfo, LogonType, Session, SessionId, Token};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::os::fd::{BorrowedFd, OwnedFd};
use uucore::error::{UResult, USimpleError};
use uucore::sid_render::{self, SidStyle};

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
    let (name, sub) = matches.subcommand().ok_or_else(|| {
        USimpleError::new(
            1,
            "logonse: subcommand required (list|show|create|destroy|end|psb)",
        )
    })?;
    let json_mode = sub.get_flag("json");
    let res = match name {
        "list" => cmd_list(json_mode),
        "show" => cmd_show(sub, json_mode),
        "create" => cmd_create(sub, json_mode),
        "destroy" => cmd_destroy(sub, json_mode),
        "end" => cmd_end(sub, json_mode),
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
                .about("List the live logon sessions and the processes in each")
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("show")
                .about("Show one logon session and the processes in it")
                .arg(
                    Arg::new("session-id")
                        .required(true)
                        .help("Session id")
                        .value_parser(clap::value_parser!(u64)),
                )
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("create")
                .about("Create a new logon session (privileged)")
                .arg(
                    Arg::new("logon-type")
                        .long("logon-type")
                        .required(true)
                        .value_name("TYPE")
                        .help(
                            "Logon type: interactive|network|batch|service|\
                             network-cleartext|new-credentials|remote-interactive",
                        ),
                )
                .arg(
                    Arg::new("auth-package")
                        .long("auth-package")
                        .required(true)
                        .value_name("STR")
                        .help("Authentication package name"),
                )
                .arg(
                    Arg::new("user-sid")
                        .long("user-sid")
                        .required(true)
                        .value_name("SID")
                        .help("User SID (e.g. S-1-5-… or an SDDL alias like BA)"),
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
            Command::new("end")
                .about(
                    "Sign a session out: authd ends every process in it. Your own sessions, \
                     and anyone's SessionEndSecurity allows",
                )
                .arg(
                    Arg::new("session-id")
                        .required(true)
                        .help("Session id")
                        .value_parser(clap::value_parser!(u64)),
                )
                .arg(
                    Arg::new("check")
                        .long("check")
                        .help("Only ask whether you may; end nothing")
                        .action(ArgAction::SetTrue),
                )
                .arg(json_flag()),
        )
        .subcommand(
            Command::new("psb")
                .about(
                    "Show a process's Process Security Block, or turn mitigations on with \
                     --mitigations",
                )
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
                        .value_name("MASK")
                        .help("Mitigation bitmask to turn on (hex or decimal)"),
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
// list / show.
// ---------------------------------------------------------------------------

/// What `list` and `show` know: the kernel's listing when the caller may read
/// it, and the processes of each session the caller may inspect.
struct Sessions {
    /// `Err` when the listing was refused (or securityfs is not mounted):
    /// the reason, said to the person.
    listing: Result<Vec<LogonSessionInfo>, String>,
    pids: BTreeMap<u64, Vec<i32>>,
}

fn gather() -> Result<Sessions, String> {
    let listing = Session::list().map_err(|e| match e.raw_os_error() {
        Some(libc::EACCES) => {
            "the kernel's list of sessions is readable only by Administrators and SYSTEM"
                .to_string()
        }
        Some(libc::ENOENT) => "securityfs is not mounted at /sys/kernel/security".to_string(),
        _ => format!("reading the kernel's list of sessions: {e}"),
    });
    Ok(Sessions {
        listing,
        pids: processes_by_session()?,
    })
}

/// Every process the caller may inspect, by the logon session of its token.
fn processes_by_session() -> Result<BTreeMap<u64, Vec<i32>>, String> {
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
    for pids in out.values_mut() {
        pids.sort_unstable();
    }
    Ok(out)
}

/// The logon session of `pid`'s token, through `/proc/<pid>/token`. `None`
/// when the caller may not read it or the process is gone.
fn session_id_for_pid(pid: i32) -> Option<u64> {
    let file = fs::File::open(format!("/proc/{pid}/token")).ok()?;
    Token::from(OwnedFd::from(file)).auth_id().ok().map(|v| v.0)
}

fn logon_type_word(raw: u32) -> String {
    match LogonType::from_raw(raw) {
        Some(LogonType::Interactive) => "interactive".into(),
        Some(LogonType::Network) => "network".into(),
        Some(LogonType::Batch) => "batch".into(),
        Some(LogonType::Service) => "service".into(),
        Some(LogonType::NetworkCleartext) => "network-cleartext".into(),
        Some(LogonType::NewCredentials) => "new-credentials".into(),
        Some(LogonType::RemoteInteractive) => "remote-interactive".into(),
        None => format!("type {raw}"),
    }
}

fn unix_seconds(at: std::time::SystemTime) -> u64 {
    at.duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn created_words(at: std::time::SystemTime) -> String {
    // The kernel makes SYSTEM's and Anonymous's sessions before the clock
    // is set.
    if unix_seconds(at) == 0 {
        return "when the machine started".into();
    }
    jiff::Timestamp::from_second(unix_seconds(at) as i64).map_or_else(
        |_| unix_seconds(at).to_string(),
        |t| {
            t.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%Y-%m-%d %H:%M:%S")
                .to_string()
        },
    )
}

fn session_json(s: &LogonSessionInfo, pids: &[i32]) -> serde_json::Value {
    json!({
        "session_id": s.id.0,
        "user_sid": s.user.to_string(),
        "user": sid_render::render(&s.user, SidStyle::Label),
        "logon_type": logon_type_word(s.logon_type),
        "auth_package": s.auth_package,
        "created_at": unix_seconds(s.created_at),
        "pids": pids,
    })
}

fn print_session(s: &LogonSessionInfo, pids: &[i32]) {
    println!(
        "session {}  {}  {}",
        s.id.0,
        sid_render::render(&s.user, SidStyle::Both),
        logon_type_word(s.logon_type)
    );
    if !s.auth_package.is_empty() {
        println!("  package: {}", s.auth_package);
    }
    println!("  created: {}", created_words(s.created_at));
    if pids.is_empty() {
        println!("  pids:    none you can see");
    } else {
        println!("  pids:    {pids:?}");
    }
}

fn cmd_list(json_mode: bool) -> Result<(), String> {
    let found = gather()?;
    match &found.listing {
        Ok(listing) => {
            if json_mode {
                let arr: Vec<_> = listing
                    .iter()
                    .map(|s| session_json(s, found.pids.get(&s.id.0).map_or(&[], |p| p)))
                    .collect();
                println!("{}", serde_json::to_string_pretty(&arr).unwrap());
                return Ok(());
            }
            for s in listing {
                print_session(s, found.pids.get(&s.id.0).map_or(&[], |p| p));
            }
            Ok(())
        }
        Err(why) => {
            // Only the sessions of processes the caller may inspect.
            if json_mode {
                let arr: Vec<_> = found
                    .pids
                    .iter()
                    .map(|(id, pids)| json!({ "session_id": id, "pids": pids }))
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({ "partial": why, "sessions": arr }))
                        .unwrap()
                );
                return Ok(());
            }
            eprintln!("logonse: {why}; showing the sessions of processes you can inspect");
            for (id, pids) in &found.pids {
                println!("session {id}  pids: {pids:?}");
            }
            Ok(())
        }
    }
}

fn cmd_show(sub: &clap::ArgMatches, json_mode: bool) -> Result<(), String> {
    let want: u64 = *sub.get_one::<u64>("session-id").unwrap();
    let found = gather()?;
    let pids = found.pids.get(&want).cloned().unwrap_or_default();
    match &found.listing {
        Ok(listing) => {
            let s = listing
                .iter()
                .find(|s| s.id.0 == want)
                .ok_or_else(|| format!("no session {want}"))?;
            if json_mode {
                println!("{}", serde_json::to_string_pretty(&session_json(s, &pids)).unwrap());
            } else {
                print_session(s, &pids);
            }
            Ok(())
        }
        Err(why) => {
            if pids.is_empty() {
                return Err(format!("{why}, and no process you can inspect is in session {want}"));
            }
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({ "partial": why, "session_id": want, "pids": pids })
                    )
                    .unwrap()
                );
            } else {
                eprintln!("logonse: {why}; showing only the processes you can inspect");
                println!("session {want}  pids: {pids:?}");
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// create / destroy.
// ---------------------------------------------------------------------------

fn cmd_create(sub: &clap::ArgMatches, json_mode: bool) -> Result<(), String> {
    let logon_type_str = sub.get_one::<String>("logon-type").unwrap();
    let logon_type = parse_logon_type(logon_type_str)?;
    let auth_package = sub.get_one::<String>("auth-package").unwrap();
    let user_sid_str = sub.get_one::<String>("user-sid").unwrap();
    let user_sid: Sid = user_sid_str
        .parse()
        .map_err(|e| format!("bad user-sid `{user_sid_str}`: {e}"))?;

    let session_id = Session::create(logon_type, auth_package, &user_sid)
        .map_err(|e| format!("session create: {e}"))?;
    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "session_id": session_id.0,
                "logon_type": logon_type_str,
                "auth_package": auth_package,
                "user_sid": user_sid_str,
            }))
            .unwrap()
        );
    } else {
        println!("created session {}", session_id.0);
    }
    Ok(())
}

fn parse_logon_type(s: &str) -> Result<LogonType, String> {
    match s.to_ascii_lowercase().replace('_', "-").as_str() {
        "interactive" => Ok(LogonType::Interactive),
        "network" => Ok(LogonType::Network),
        "batch" => Ok(LogonType::Batch),
        "service" => Ok(LogonType::Service),
        "network-cleartext" => Ok(LogonType::NetworkCleartext),
        "new-credentials" => Ok(LogonType::NewCredentials),
        "remote-interactive" => Ok(LogonType::RemoteInteractive),
        other => Err(format!(
            "unknown logon-type `{other}` (expected one of: interactive, network, \
             batch, service, network-cleartext, new-credentials, remote-interactive)"
        )),
    }
}

fn cmd_destroy(sub: &clap::ArgMatches, json_mode: bool) -> Result<(), String> {
    let id: u64 = *sub.get_one::<u64>("session-id").unwrap();
    Session::destroy_empty(SessionId(id)).map_err(|e| format!("session destroy: {e}"))?;
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

// ---------------------------------------------------------------------------
// end.
// ---------------------------------------------------------------------------

/// Signs a session out, through authd (PGSS Logon §2.22): the kernel has no
/// call that ends a session's processes, and who may is authd's to decide.
fn cmd_end(sub: &clap::ArgMatches, json_mode: bool) -> Result<(), String> {
    let id: u64 = *sub.get_one::<u64>("session-id").unwrap();
    let logon = Logon::new();
    let refused = |refusal: Refusal| {
        if refusal.not_permitted() {
            // authd says the same of a session nobody may end as of one this
            // caller may not; the kernel's list, where it can be read, tells
            // which.
            let listed = Session::list().ok().and_then(|all| all.into_iter().find(|s| s.id.0 == id));
            match listed {
                // SYSTEM's and Anonymous's, which are listed as services' too.
                Some(_) if id == 998 || id == 999 => format!("session {id}: the kernel's own sessions never end"),
                Some(s) if LogonType::from_raw(s.logon_type) == Some(LogonType::Service) => format!(
                    "session {id}: a service's session ends when its service is stopped (svctl stop)"
                ),
                _ => format!(
                    "session {id}: you may not end it: only its own person, and whoever \
                     SessionEndSecurity allows, may"
                ),
            }
        } else if refusal.no_such_session() {
            format!("session {id}: no such session")
        } else {
            format!("session {id}: {refusal}")
        }
    };
    if sub.get_flag("check") {
        logon.may_end_session(id).map_err(refused)?;
        if json_mode {
            println!("{}", serde_json::to_string_pretty(&json!({ "session_id": id, "may_end": true })).unwrap());
        } else {
            println!("you may end session {id}");
        }
        return Ok(());
    }
    let ended = logon.end_session(id).map_err(refused)?;
    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "session_id": id,
                "ended": ended.ended,
                "remaining": ended.remaining,
            }))
            .unwrap()
        );
    } else if ended.remaining == 0 {
        println!("ended session {id}: {} processes ended", ended.ended);
    } else {
        println!(
            "session {id}: {} processes ended, {} could not be and still hold it",
            ended.ended, ended.remaining
        );
    }
    if ended.remaining == 0 { Ok(()) } else { Err(format!("session {id} is still open")) }
}

// ---------------------------------------------------------------------------
// psb.
// ---------------------------------------------------------------------------

fn cmd_psb(sub: &clap::ArgMatches, json_mode: bool) -> Result<(), String> {
    let pid: i32 = *sub.get_one::<i32>("pid").unwrap();
    let Some(mitigations_str) = sub.get_one::<String>("mitigations") else {
        return show_psb(pid, json_mode);
    };
    let mitigations = parse_mask(mitigations_str)?;

    let pidfd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if pidfd < 0 {
        return Err(format!(
            "pidfd_open(pid={pid}): {}",
            std::io::Error::last_os_error()
        ));
    }
    let pidfd = pidfd as i32;
    // SAFETY: `pidfd` is a valid fd we own for the duration of this borrow;
    // we close it below after `set_mitigations` returns.
    let borrowed = unsafe { BorrowedFd::borrow_raw(pidfd) };
    let r = Process::set_mitigations(Some(borrowed), Mitigations::from_bits_retain(mitigations));
    unsafe {
        let _ = libc::close(pidfd);
    }
    r.map_err(|e| format!("set_mitigations: {e}"))?;

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

/// The names of the mitigation bits set in `m`, in bit order.
fn mitigation_words(m: Mitigations) -> Vec<&'static str> {
    [
        (Mitigations::WXP, "wxp"),
        (Mitigations::TLP, "tlp"),
        (Mitigations::LSV, "lsv"),
        (Mitigations::UI_ACCESS, "ui-access"),
        (Mitigations::NO_CHILD, "no-child"),
        (Mitigations::CFIF, "cfif"),
        (Mitigations::CFIB, "cfib"),
        (Mitigations::PIE, "pie"),
        (Mitigations::SML, "sml"),
    ]
    .into_iter()
    .filter(|(bit, _)| m.contains(*bit))
    .map(|(_, word)| word)
    .collect()
}

fn show_psb(pid: i32, json_mode: bool) -> Result<(), String> {
    let wanted = u32::try_from(pid)
        .ok()
        .filter(|&p| p > 0)
        .ok_or_else(|| format!("bad pid {pid}"))?;
    let psb = Process::psb(Some(wanted)).map_err(|e| format!("psb of {pid}: {e}"))?;
    let mut guid = String::new();
    for (i, b) in psb.process_guid.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            guid.push('-');
        }
        guid.push_str(&format!("{b:02x}"));
    }
    let words = mitigation_words(psb.mitigations);
    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "pid": pid,
                "pip_type": psb.pip_type,
                "pip_trust": psb.pip_trust,
                "mitigations": format!("0x{:03x}", psb.mitigations.bits()),
                "mitigation_names": words,
                "process_guid": guid,
            }))
            .unwrap()
        );
        return Ok(());
    }
    let protection = match (psb.pip_type, psb.pip_trust) {
        (0, _) => "none".to_string(),
        (512, 8192) => "protected, Peios TCB".to_string(),
        (t, trust) => format!("type {t}, trust {trust}"),
    };
    println!("psb pid={pid}");
    println!("  pip:         {protection}");
    if words.is_empty() {
        println!("  mitigations: none");
    } else {
        println!(
            "  mitigations: {} (0x{:03x})",
            words.join(", "),
            psb.mitigations.bits()
        );
    }
    println!("  guid:        {guid}");
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
