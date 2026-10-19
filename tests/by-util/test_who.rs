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
