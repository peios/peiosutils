//! Best-effort per-thread inspection; failures remain visible in the output.
use crate::cmd;
use crate::error::{Error, Result};
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::sid_render::{self, SidStyle};
use crate::target::TargetSpec;
use peios::security::Sid;
use peios::token::{TokenAccess, TokenType};
use serde_json::json;
use std::path::Path;

fn directory_error(path: &Path, error: std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        Error::NotFound(path.display().to_string())
    } else {
        Error::Syscall {
            op: "enumerate threads",
            errno: error.raw_os_error().unwrap_or(libc::EIO),
            detail: Some(path.display().to_string()),
        }
    }
}

fn thread_ids(path: &Path) -> Result<Vec<i32>> {
    let mut tids = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|e| directory_error(path, e))? {
        let entry = entry.map_err(|e| directory_error(path, e))?;
        if let Some(tid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<i32>().ok())
            .filter(|tid| *tid > 0)
        {
            tids.push(tid);
        }
    }
    tids.sort_unstable();
    Ok(tids)
}

fn output(
    pid: i32,
    tids: &[i32],
    style: SidStyle,
    mut inspect: impl FnMut(i32) -> Result<(bool, Sid)>,
) -> (CmdOutput, Option<Error>) {
    let mut human = Lines::new();
    human.kv("TID", "impersonating  user");
    let mut rows = Vec::new();
    let mut failure = None;
    for &tid in tids {
        match inspect(tid) {
            Ok((impersonating, user)) => {
                human.kv(
                    tid.to_string(),
                    format!(
                        "{:<13} {}",
                        if impersonating { "yes" } else { "no" },
                        sid_render::render(&user, style)
                    ),
                );
                rows.push(json!({ "tid": tid, "impersonating": impersonating, "user": sid_render::render_json(&user) }));
            }
            Err(error) => {
                let message = error.to_string();
                human.kv(
                    tid.to_string(),
                    format!("{:<13} error: {}", "?", message.replace('\n', "; ")),
                );
                rows.push(
                    json!({ "tid": tid, "impersonating": null, "user": null, "error": message }),
                );
                if failure.is_none() {
                    failure = Some(error);
                }
            }
        }
    }
    (
        CmdOutput {
            human,
            json: json!({ "pid": pid, "complete": failure.is_none(), "threads": rows }),
        },
        failure,
    )
}

pub fn run(pid: i32, style: SidStyle, mode: OutputMode) -> Result<()> {
    let tids = thread_ids(Path::new(&format!("/proc/{pid}/task")))?;
    let (out, failure) = output(pid, &tids, style, |tid| {
        let token = TargetSpec::Thread { pid, tid }.open(TokenAccess::QUERY.bits())?;
        Ok((
            token.token_type()? == TokenType::Impersonation,
            token.user()?,
        ))
    });
    cmd::emit(out, mode)?;
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_numeric_thread_ids_and_preserves_enumeration_errors() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["31", "7", "12", "not-a-tid"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        assert_eq!(thread_ids(dir.path()).unwrap(), vec![7, 12, 31]);
        assert!(matches!(
            thread_ids(&dir.path().join("gone")),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn reports_primary_impersonating_and_disappeared_threads_without_guessing() {
        let user = Sid::build(5, &[18]).unwrap();
        let (out, failure) = output(7, &[7, 8, 9], SidStyle::Both, |tid| match tid {
            7 => Ok((false, user)),
            8 => Ok((true, user)),
            _ => Err(Error::NotFound("pid 7 tid 9".into())),
        });
        assert_eq!(out.json["threads"][0]["impersonating"], false);
        assert_eq!(out.json["threads"][1]["impersonating"], true);
        assert!(out.json["threads"][2]["impersonating"].is_null());
        assert!(
            out.json["threads"][2]["error"]
                .as_str()
                .unwrap()
                .contains("tid 9")
        );
        assert_eq!(out.json["complete"], false);
        assert_eq!(failure.unwrap().exit_code(), 2);
    }
}
