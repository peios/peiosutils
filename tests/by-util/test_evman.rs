// Integration tests for `evman`, driving the real multicall binary against a
// throwaway catalogue. evman reads only on-disk fragments (never the event
// stream), so it runs fully in CI with no kernel.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_peiosutils");

/// A cut-down platform fragment: a few generic fields and the subject group.
const KERNEL_FRAGMENT: &str = "\
--- field subject.token.sid
type: bin.sid

The user SID of the effective token the operation ran under. Under
impersonation this is the client's SID.

--- field object.kind
type: str.enum
values: file | process | token
closed: false

What kind of object the event is about.

--- field access.granted
type: uint.mask

The access mask the check granted.

--- field outcome.success
type: bool

Whether the action succeeded.

--- group subject
include: subject.token.sid

The identity of whoever acted.
";

const KACS_FRAGMENT: &str = "\
--- event kacs.audit.access.checked
tier: essential
gating: a matching SACL audit ACE

The record that an access check completed, and what it decided. The most
common event on the system.

include: subject
field: object.kind              required
field: access.granted           required
  The mask this check granted.
field: outcome.success          required
";

fn corpus() -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("kernel.evman"), KERNEL_FRAGMENT).unwrap();
    std::fs::write(tmp.path().join("kacs.evman"), KACS_FRAGMENT).unwrap();
    tmp
}

fn evman(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .arg("evman")
        .args(args)
        .env("EVMAN_DIR", dir)
        .env_remove("PAGER")
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn event_lookup_renders_the_event_card() {
    let tmp = corpus();
    let out = evman(tmp.path(), &["kacs.audit.access.checked"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("defined by kacs"));
    assert!(s.contains("Tier"));
    assert!(s.contains("essential"));
    assert!(s.contains("The most common event on the system."));
    assert!(s.contains("Fields"));
    assert!(s.contains("subject.token.sid"));
    assert!(s.contains("The mask this check granted."));
}

#[test]
fn field_lookup_lists_its_carriers() {
    let tmp = corpus();
    let out = evman(tmp.path(), &["access.granted"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("uint.mask"));
    assert!(s.contains("Carried by"));
    assert!(s.contains("kacs.audit.access.checked"));
}

#[test]
fn prefix_lookup_is_an_index() {
    let tmp = corpus();
    let out = evman(tmp.path(), &["kacs.audit"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("Event types"));
    assert!(s.contains("The record that an access check completed, and what it decided."));
    assert!(!s.contains("The most common event"));
}

#[test]
fn a_variant_resolves_to_its_field() {
    let tmp = corpus();
    let out = evman(tmp.path(), &["outcome.success-previous"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("-previous variant of outcome.success"));
    assert!(s.contains("Whether the action succeeded."));
}

#[test]
fn unknown_name_exits_2() {
    let tmp = corpus();
    let out = evman(tmp.path(), &["kacs.nothing.happened"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn apropos_searches_names_and_summaries() {
    let tmp = corpus();
    let out = evman(tmp.path(), &["-k", "access"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("access.granted"));
    assert!(s.contains("kacs.audit.access.checked"));
    assert!(!s.contains("subject.token.sid"));
}

#[test]
fn lint_accepts_a_clean_catalogue() {
    let tmp = corpus();
    let k = tmp.path().join("kernel.evman");
    let a = tmp.path().join("kacs.evman");
    let out = evman(tmp.path(), &["lint", k.to_str().unwrap(), a.to_str().unwrap()]);
    assert!(out.status.success(), "lint stdout: {}", stdout(&out));
}

#[test]
fn lint_reports_findings_and_exits_3() {
    let tmp = corpus();
    // Linted alone, kacs.evman carries fields nothing defines.
    let a = tmp.path().join("kacs.evman");
    let out = evman(tmp.path(), &["lint", a.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(3));
    let s = stdout(&out);
    assert!(s.contains("kacs.evman:9: rule 1: kacs.audit.access.checked carries undefined object.kind"));
    assert!(s.contains("kacs.evman:8: rule 1: kacs.audit.access.checked includes undefined group subject"));
}

#[test]
fn lookup_warns_of_a_fragment_breaking_rule_2() {
    let tmp = corpus();
    std::fs::write(
        tmp.path().join("lcs.evman"),
        "--- field outcome.success\ntype: bool\n\nAgain.\n",
    )
    .unwrap();
    let out = evman(tmp.path(), &["outcome.success"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("lcs.evman breaks rule 2"));
    // The first definition is the one shown.
    assert!(stdout(&out).contains("Whether the action succeeded."));
}
