// The driven mode (--driven): how a program, rather than a person at a
// terminal, runs upgrade-peios. Upgrade Peios is the first.
//
// The package half is peipkg's own driven mode, run with this process's
// standard input, so the driver answers peipkg's questions under peipkg's
// own ids and upgrade-peios reads nothing. Every event peipkg writes is
// passed on as it is, except its terminal event: upgrade-peios is not done
// when peipkg is. It then adds its own progress for the release's seeds and
// ends with one terminal event of its own — done, cancelled or error.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::Error;

/// How peipkg's part ended, when it didn't fail.
pub enum Ended {
    /// peipkg's summary, such as "nothing to do".
    Done(Option<String>),
    Cancelled(String),
}

/// Write one event as a line. A driver that has gone away loses the
/// events, not the upgrade: the write fails quietly (SIGPIPE is ignored).
pub fn event(v: &Value) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

pub fn message(text: &str) {
    event(&json!({"event": "message", "text": text}));
}

/// A step of the release's own part: `phase` is release-stage (copying the
/// seeds into the queue) or release-apply (applying them).
pub fn progress(phase: &str, step: usize, steps: usize) {
    event(&json!({"event": "progress", "phase": phase, "step": step, "steps": steps}));
}

/// The terminal event of an upgrade-peios that failed.
pub fn error(err: &Error) {
    event(&json!({"event": "error", "code": err.code(), "message": err.to_string()}));
}

/// Run `cmd`, peipkg with --driven, passing its events on until its
/// terminal one, which is returned instead.
pub fn peipkg(mut cmd: Command) -> Result<Ended, Error> {
    let mut child = cmd
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Usage(format!("cannot run peipkg: {e}")))?;
    let stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    // Read beside the events, so a peipkg that writes a lot of it can't
    // stall on a full pipe.
    let fault = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let mut end = None;
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        let parsed: Option<Value> = serde_json::from_str(&line).ok();
        let kind = parsed.as_ref().and_then(|v| v.get("event")).and_then(Value::as_str);
        let text = |key: &str| parsed.as_ref().and_then(|v| v.get(key)).and_then(Value::as_str).map(str::to_string);
        match kind {
            Some("done") => end = Some(Ok(Ended::Done(text("summary")))),
            Some("cancelled") => end = Some(Ok(Ended::Cancelled(text("reason").unwrap_or_default()))),
            Some("error") => {
                end = Some(Err(Error::Refused {
                    code: text("code").unwrap_or_else(|| "failed".into()),
                    message: text("message").unwrap_or_default(),
                }));
            }
            _ => {
                let mut out = io::stdout().lock();
                let _ = writeln!(out, "{line}");
                let _ = out.flush();
            }
        }
    }
    let fault = fault.join().unwrap_or_default();
    let _ = child.wait();
    end.unwrap_or_else(|| {
        let said = fault.lines().rev().find(|l| !l.trim().is_empty()).map(str::trim);
        Err(Error::Refused {
            code: "failed".into(),
            message: said.unwrap_or("peipkg stopped without saying how it ended").to_string(),
        })
    })
}

/// Run `cmd` to its end, each line it writes on standard output or error
/// sent as a message.
pub fn relayed(mut cmd: Command) -> io::Result<std::process::ExitStatus> {
    let out = cmd.stdin(Stdio::null()).output()?;
    for line in String::from_utf8_lossy(&out.stdout).lines().chain(String::from_utf8_lossy(&out.stderr).lines()) {
        if !line.trim().is_empty() {
            message(line.trim_end());
        }
    }
    Ok(out.status)
}
