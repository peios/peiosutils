// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::new_ucmd;

#[test]
fn test_help() {
    new_ucmd!()
        .arg("--help")
        .succeeds()
        .stdout_contains("Change your own password");
}

/// There is no way to name another principal — the protocol has no field for
/// one — so a name gets an answer pointing at the tool that can, rather than
/// clap's generic complaint.
#[test]
fn test_a_name_is_refused_with_a_pointer() {
    new_ucmd!()
        .arg("alice")
        .fails_with_code(2)
        .stderr_contains("only your own password")
        .stderr_contains("lps password");
}

/// Off a Peios machine there is no authority to reach, and saying so is the
/// whole of the answer.
#[test]
#[cfg(unix)]
fn test_no_authority_is_a_failure_that_names_it() {
    if std::path::Path::new("/run/logon.sock").exists() {
        return;
    }
    new_ucmd!()
        .fails_with_code(1)
        .stderr_contains("cannot reach the authority");
}
