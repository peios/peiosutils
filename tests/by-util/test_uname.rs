// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
use uutests::at_and_ucmd;
use uutests::new_ucmd;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails_with_code(1);
}

#[test]
fn test_uname() {
    new_ucmd!().succeeds();
}

#[test]
fn test_uname_compatible() {
    new_ucmd!().arg("-a").succeeds();
}

#[test]
fn test_uname_name() {
    new_ucmd!().arg("-n").succeeds();
}

#[test]
fn test_uname_processor() {
    let result = new_ucmd!().arg("-p").succeeds();
    assert_eq!(result.stdout_str().trim_end(), "unknown");
}

#[test]
fn test_uname_hardware_platform() {
    new_ucmd!()
        .arg("-i")
        .succeeds()
        .stdout_str_apply(str::trim_end)
        .stdout_only("unknown");
}

#[test]
fn test_uname_machine() {
    new_ucmd!().arg("-m").succeeds();
}

#[test]
fn test_uname_kernel_version() {
    new_ucmd!().arg("-v").succeeds();
}

#[test]
fn test_uname_kernel() {
    let (_, mut ucmd) = at_and_ucmd!();
    let result = ucmd.arg("-s").succeeds();
    #[cfg(target_os = "linux")]
    result.stdout_contains("Linux");
}

#[test]
fn test_uname_operating_system() {
    new_ucmd!()
        .arg("--operating-system")
        .succeeds()
        .stdout_is("Peios\n");
}

#[test]
fn test_uname_help() {
    new_ucmd!()
        .arg("--help")
        .succeeds()
        .stdout_contains("system information");
}

#[test]
fn test_uname_output_for_invisible_chars() {
    // let re = regex::Regex::new("[^[[:print:]]]").unwrap(); // matches invisible (and emojis)
    let re = regex::Regex::new("[^[[:print:]]\\p{Other_Symbol}]").unwrap(); // matches invisible (not emojis)
    let result = new_ucmd!().arg("--all").succeeds();
    assert_eq!(re.find(result.stdout_str().trim_end()), None);
}
