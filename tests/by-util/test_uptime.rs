// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
//
// spell-checker:ignore utmp runlevel testusr testx boottime
#![allow(clippy::cast_possible_wrap, clippy::unreadable_literal)]

use uutests::{at_and_ucmd, new_ucmd};

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails_with_code(1);
}

#[test]
fn test_uptime() {
    new_ucmd!()
        .succeeds()
        .stdout_contains("load average:")
        .stdout_contains(" up ");

    // Don't check for users as it doesn't show in some CI
}

#[test]
#[cfg(target_os = "linux")]
fn test_write_error_handling() {
    use std::fs::File;

    let dev_full =
        File::create("/dev/full").expect("Failed to open /dev/full - test must run on Linux");

    new_ucmd!()
        .set_stdout(dev_full)
        .fails()
        .code_is(1)
        .stderr_contains("No space left on device");
}

#[test]
fn utmp_file_input_is_explicitly_deferred() {
    let (files, mut command) = at_and_ucmd!();
    files.write("session-data", "not a Peios session database");
    command
        .arg("session-data")
        .fails_with_code(1)
        .no_stdout()
        .stderr_contains("deferred on Peios");
    assert_eq!(files.read("session-data"), "not a Peios session database");
}

#[test]
fn missing_utmp_file_is_also_deferred() {
    new_ucmd!()
        .arg("missing-session-data")
        .fails_with_code(1)
        .stderr_contains("deferred on Peios");
}
