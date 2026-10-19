// This file is part of the peiosutils package.
// See LICENSE for copyright and license information.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

struct OfflineUpgrade {
    directory: TempDir,
    root: PathBuf,
    tools: PathBuf,
    arguments: PathBuf,
}

impl OfflineUpgrade {
    fn new(reply: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("root");
        let tools = directory.path().join("tools");
        let arguments = directory.path().join("arguments");
        for path in ["usr/lib", "usr/share/peios", "usr/share/regim"] {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        fs::create_dir(&tools).unwrap();
        fs::write(
            root.join("usr/lib/os-release"),
            "ID=peios\nVARIANT_ID=experimental\n",
        )
        .unwrap();
        fs::write(
            root.join("usr/share/peios/release.toml"),
            "[registry]\nautoapply = [\"example\"]\n",
        )
        .unwrap();
        fs::write(root.join("usr/share/regim/example.reg"), "seed contents\n").unwrap();
        let script = tools.join("peipkg");
        fs::write(&script, format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$UPGRADE_TEST_ARGS\"\nprintf '%s\\n' '{reply}'\n")).unwrap();
        fs::set_permissions(script, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            directory,
            root,
            tools,
            arguments,
        }
    }

    fn run(&self, options: &[&str]) -> Output {
        let binary =
            std::env::var_os("UUTESTS_BINARY_PATH").unwrap_or_else(|| crate::TESTS_BINARY.into());
        Command::new(binary)
            .args(["upgrade-peios", "--driven", "--root"])
            .arg(&self.root)
            .args(options)
            .env("PATH", &self.tools)
            .env("UPGRADE_TEST_ARGS", &self.arguments)
            .current_dir(self.directory.path())
            .output()
            .unwrap()
    }

    fn events(&self, output: &Output) -> Vec<Value> {
        assert!(output.stderr.is_empty(), "{output:?}");
        String::from_utf8(output.stdout.clone())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn assert_no_seeds_staged(&self) {
        assert!(!self.root.join("lcl").exists());
    }
}

#[test]
fn cancellation_finishes_without_staging_seeds() {
    let upgrade = OfflineUpgrade::new(r#"{"event":"cancelled","reason":"declined"}"#);
    let output = upgrade.run(&[]);
    assert!(output.status.success(), "{output:?}");
    let events = upgrade.events(&output);
    assert_eq!(
        events
            .iter()
            .map(|event| event["event"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["message", "cancelled"]
    );
    assert_eq!(events[1]["reason"], "declined");
    upgrade.assert_no_seeds_staged();
}

#[test]
fn checking_finishes_without_staging_seeds() {
    let upgrade = OfflineUpgrade::new(r#"{"event":"done","summary":"current"}"#);
    let output = upgrade.run(&["--check"]);
    assert!(output.status.success(), "{output:?}");
    let events = upgrade.events(&output);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["event"], "done");
    assert_eq!(events[1]["summary"], "current");
    assert!(
        fs::read_to_string(&upgrade.arguments)
            .unwrap()
            .lines()
            .any(|line| line == "--dry-run")
    );
    upgrade.assert_no_seeds_staged();
}

#[test]
fn offline_upgrade_stages_seeds_after_package_completion() {
    let upgrade = OfflineUpgrade::new(r#"{"event":"done","summary":"installed"}"#);
    let output = upgrade.run(&[]);
    assert!(output.status.success(), "{output:?}");
    let events = upgrade.events(&output);
    assert_eq!(
        events
            .iter()
            .map(|event| event["event"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["message", "progress", "message", "message", "done"]
    );
    assert_eq!(events[1]["phase"], "release-stage");
    assert_eq!(events[1]["steps"], 1);
    assert_eq!(events[4]["seeds_queued"], true);
    assert_eq!(
        fs::read(upgrade.root.join("lcl/policy/autoapply.d/example.reg")).unwrap(),
        b"seed contents\n"
    );
    assert!(
        upgrade
            .root
            .join("lcl/policy/autorun.d/10-apply-seeds.sh")
            .is_file()
    );
}

#[test]
fn package_failure_does_not_stage_seeds() {
    let upgrade = OfflineUpgrade::new(r#"{"event":"error","code":"denied","message":"refused"}"#);
    let output = upgrade.run(&[]);
    assert_eq!(output.status.code(), Some(3));
    let events = upgrade.events(&output);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["event"], "error");
    assert_eq!(events[1]["code"], "denied");
    upgrade.assert_no_seeds_staged();
}

#[test]
fn seeds_only_does_not_run_the_package_manager() {
    let upgrade =
        OfflineUpgrade::new(r#"{"event":"error","code":"unexpected","message":"must not run"}"#);
    let output = upgrade.run(&["--seeds-only"]);
    assert!(output.status.success(), "{output:?}");
    assert!(!upgrade.arguments.exists());
    assert_eq!(
        fs::read(upgrade.root.join("lcl/policy/autoapply.d/example.reg")).unwrap(),
        b"seed contents\n"
    );
}
