// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::new_ucmd;

#[test]
fn unavailable_interface_reports_deferral() {
    for arguments in [
        vec![],
        vec!["--help"],
        vec!["--definitely-invalid"],
        vec!["sample"],
    ] {
        new_ucmd!()
            .args(&arguments)
            .fails_with_code(1)
            .no_stdout()
            .stderr_contains("deferred on Peios");
    }
}

#[test]
fn deferred_install_leaves_source_and_destination_unchanged() {
    let (files, mut command) = uutests::at_and_ucmd!();
    files.write("source", "source contents");
    files.write("destination", "existing contents");
    command.args(&["source", "destination"]).fails_with_code(1);
    assert_eq!(files.read("source"), "source contents");
    assert_eq!(files.read("destination"), "existing contents");
}
