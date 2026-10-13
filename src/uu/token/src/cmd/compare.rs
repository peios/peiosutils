//! Comparison of the selected token fields, using the ordinary query path.
use crate::cmd;
use crate::error::{Error, Result};
use crate::payload::{self, SidAndAttrsEntry};
use crate::privs::{self, PrivSnapshot};
use crate::render::{CmdOutput, Lines, OutputMode};
use crate::sid_render::{self, SidStyle};
use crate::target::TargetSpec;
use clap::ArgMatches;
use peios::security::Sid;
use peios::token::{TokenAccess, TokenClass};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn operand(value: &str) -> Result<TargetSpec> {
    if value == "self" {
        return Ok(TargetSpec::SelfTok { real: false });
    }
    match value.parse::<i32>() {
        Ok(pid) if pid > 0 => Ok(TargetSpec::Pid(pid)),
        _ => Err(Error::Usage(format!(
            "invalid comparison target {value:?}: expected PID or self"
        ))),
    }
}

fn targets(args: &ArgMatches) -> Result<(TargetSpec, TargetSpec)> {
    let left = operand(args.get_one::<String>("left").expect("required target"))?;
    let flags = args.get_flag("self")
        || args.get_flag("real")
        || ["pid", "tid", "peer"]
            .iter()
            .any(|key| args.get_one::<i32>(key).is_some());
    let right = match (args.get_one::<String>("right"), flags) {
        (Some(value), false) => operand(value)?,
        (None, true) => cmd::parse_target(args)?,
        (Some(_), true) => {
            return Err(Error::Usage(
                "choose RIGHT or right-hand target flags, not both".into(),
            ));
        }
        (None, false) => {
            return Err(Error::Usage(
                "compare requires a second target or explicit target flags".into(),
            ));
        }
    };
    Ok((left, right))
}

struct Snapshot {
    user: Sid,
    integrity: Sid,
    session: u64,
    groups: Vec<SidAndAttrsEntry>,
    privileges: PrivSnapshot,
}

impl Snapshot {
    fn read(target: TargetSpec) -> Result<Self> {
        let tok = target.open(TokenAccess::QUERY.bits())?;
        Ok(Self {
            user: tok.user()?,
            integrity: cmd::show::query_sid(
                &tok,
                TokenClass(peios_sys::KACS_TOKEN_CLASS_INTEGRITY_LEVEL),
            )?,
            session: tok.auth_id()?.0,
            groups: payload::parse_sid_attrs_list(&tok.query(TokenClass::GROUPS)?)
                .map_err(Error::Decode)?,
            privileges: privs::decode_privs_payload(&tok.query(TokenClass::PRIVILEGES)?)
                .map_err(Error::Decode)?,
        })
    }
}

fn group_state(entry: Option<&&SidAndAttrsEntry>) -> Option<&'static str> {
    entry.map(|entry| {
        if entry.attributes & payload::SE_GROUP_USE_FOR_DENY_ONLY != 0 {
            "deny-only"
        } else if entry.attributes & payload::SE_GROUP_ENABLED != 0 {
            "enabled"
        } else {
            "disabled"
        }
    })
}

fn privilege_state(snapshot: &PrivSnapshot, bit: u32) -> Option<&'static str> {
    let mask = 1u64 << bit;
    (snapshot.present & mask != 0).then_some(if snapshot.enabled & mask != 0 {
        "enabled"
    } else {
        "disabled"
    })
}

fn comparison(left: Value, right: Value, matches: bool) -> Value {
    json!({ "left": left, "right": right, "matches": matches })
}

fn output(left: &Snapshot, right: &Snapshot, labels: [&str; 2], style: SidStyle) -> CmdOutput {
    let mut human = Lines::new();
    let mut rows: Vec<(String, String, String, bool)> = Vec::new();
    let mut header = json!({});
    for (key, a, b) in [
        ("user", &left.user, &right.user),
        ("integrity", &left.integrity, &right.integrity),
    ] {
        let matches = a.to_string() == b.to_string();
        header[key] = comparison(
            sid_render::render_json(a),
            sid_render::render_json(b),
            matches,
        );
        rows.push((
            key.into(),
            sid_render::render(a, style),
            sid_render::render(b, style),
            matches,
        ));
    }
    header["session_id"] = comparison(
        json!(left.session),
        json!(right.session),
        left.session == right.session,
    );
    rows.push((
        "session_id".into(),
        left.session.to_string(),
        right.session.to_string(),
        left.session == right.session,
    ));
    let mut groups = Vec::new();
    let a: BTreeMap<_, _> = left
        .groups
        .iter()
        .map(|entry| (entry.sid.to_string(), entry))
        .collect();
    let b: BTreeMap<_, _> = right
        .groups
        .iter()
        .map(|entry| (entry.sid.to_string(), entry))
        .collect();
    let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
    for key in keys {
        let l = group_state(a.get(key));
        let r = group_state(b.get(key));
        let entry = a.get(key).or_else(|| b.get(key)).expect("union member");
        let mut row = comparison(json!(l), json!(r), l == r);
        row["group"] = sid_render::render_json(&entry.sid);
        groups.push(row);
        rows.push((
            format!("group {}", sid_render::render(&entry.sid, style)),
            l.unwrap_or("-").into(),
            r.unwrap_or("-").into(),
            l == r,
        ));
    }
    let mut privileges = Vec::new();
    let union = PrivSnapshot {
        present: left.privileges.present | right.privileges.present,
        ..PrivSnapshot::default()
    };
    for entry in union.entries() {
        let l = privilege_state(&left.privileges, entry.bit);
        let r = privilege_state(&right.privileges, entry.bit);
        let mut row = comparison(json!(l), json!(r), l == r);
        row["bit"] = entry.bit.into();
        row["name"] = json!(entry.name);
        privileges.push(row);
        rows.push((
            entry.label(),
            l.unwrap_or("-").into(),
            r.unwrap_or("-").into(),
            l == r,
        ));
    }
    let width = rows
        .iter()
        .map(|row| row.1.len())
        .chain([labels[0].len()])
        .max()
        .unwrap_or(0);
    human.kv("field", format!("{:<width$}    {}", labels[0], labels[1]));
    for (key, l, r, matches) in rows {
        human.kv(
            key,
            format!("{l:<width$} {:2} {r}", if matches { "=" } else { "!=" }),
        );
    }
    CmdOutput {
        human,
        json: json!({ "targets": { "left": labels[0], "right": labels[1] }, "header": header, "groups": groups, "privileges": privileges }),
    }
}

pub fn run(args: &ArgMatches, style: SidStyle, mode: OutputMode) -> Result<()> {
    let (left, right) = targets(args)?;
    let a = Snapshot::read(left)?;
    let b = Snapshot::read(right)?;
    cmd::emit(output(&a, &b, [&left.label(), &right.label()], style), mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_task_examples_and_rejects_ambiguous_targets() {
        for (args, expected) in [
            (vec!["token", "compare", "1482", "1500"], "pid 1500"),
            (
                vec!["token", "compare", "1482", "--pid", "1482", "--tid", "1509"],
                "pid 1482 tid 1509",
            ),
            (
                vec!["token", "compare", "self", "--real"],
                "self (real/primary)",
            ),
        ] {
            let m = crate::cli::build().try_get_matches_from(args).unwrap();
            assert_eq!(
                targets(m.subcommand().unwrap().1).unwrap().1.label(),
                expected
            );
        }
        for args in [
            vec!["token", "compare", "self"],
            vec!["token", "compare", "0", "self"],
            vec!["token", "compare", "1", "2", "--real"],
            vec!["token", "compare", "1", "--tid", "2"],
        ] {
            let m = crate::cli::build().try_get_matches_from(args).unwrap();
            assert!(targets(m.subcommand().unwrap().1).is_err());
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            user: Sid::build(5, &[18]).unwrap(),
            integrity: Sid::build(16, &[8192]).unwrap(),
            session: 42,
            groups: vec![],
            privileges: PrivSnapshot::default(),
        }
    }

    #[test]
    fn comparison_preserves_missing_disabled_deny_only_and_unknown_privileges() {
        let mut a = snapshot();
        let mut b = snapshot();
        a.groups.push(SidAndAttrsEntry {
            sid: Sid::build(5, &[32, 544]).unwrap(),
            attributes: payload::SE_GROUP_ENABLED,
        });
        b.groups.push(SidAndAttrsEntry {
            sid: a.groups[0].sid,
            attributes: payload::SE_GROUP_USE_FOR_DENY_ONLY,
        });
        b.groups.push(SidAndAttrsEntry {
            sid: Sid::build(5, &[99]).unwrap(),
            attributes: 0,
        });
        a.privileges.present = 1u64 << 63;
        b.session = 43;
        let out = output(&a, &b, ["left", "right"], SidStyle::Both);
        assert_eq!(out.json["header"]["user"]["matches"], true);
        assert_eq!(out.json["header"]["session_id"]["matches"], false);
        assert_eq!(out.json["groups"][0]["left"], "enabled");
        assert_eq!(out.json["groups"][0]["right"], "deny-only");
        assert_eq!(out.json["groups"][1]["left"], Value::Null);
        assert_eq!(out.json["groups"][1]["right"], "disabled");
        assert_eq!(out.json["privileges"][0]["bit"], 63);
        assert_eq!(out.json["privileges"][0]["left"], "disabled");
        assert_eq!(out.json["privileges"][0]["right"], Value::Null);
        assert_eq!(out.json["privileges"][0]["matches"], false);
    }
}
