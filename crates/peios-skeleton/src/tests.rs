use super::*;

use tempfile::tempdir;

fn plain(at: &Path, _: Option<&str>) -> io::Result<()> {
    fs::create_dir(at)
}

fn dir(path: &str, descriptor: Option<&str>) -> Entry {
    Entry::Dir {
        path: path.to_owned(),
        descriptor: descriptor.map(str::to_owned),
    }
}

fn link(path: &str, target: &str) -> Entry {
    Entry::Symlink {
        path: path.to_owned(),
        target: target.to_owned(),
    }
}

fn rejects(text: &str, line: usize, says: &str) {
    let error = parse(text).unwrap_err();
    assert_eq!(error.line, line, "{error}");
    assert!(error.message.contains(says), "{error}");
}

#[test]
fn reads_directories_descriptors_and_links() {
    let text = "# The skeleton.\n\n/var\n\t/var/state   O:SYG:SYD:P(A;OICI;GA;;;SY)  \n/lib64 -> usr/lib/x86_64-linux-peios\n";
    let skeleton = parse(text).unwrap();
    assert_eq!(
        skeleton.entries,
        [
            dir("/var", None),
            dir("/var/state", Some("O:SYG:SYD:P(A;OICI;GA;;;SY)")),
            link("/lib64", "usr/lib/x86_64-linux-peios"),
        ]
    );
}

#[test]
fn an_empty_file_is_an_empty_skeleton() {
    assert_eq!(parse("").unwrap(), Skeleton::default());
    assert_eq!(parse("# nothing\n\n").unwrap(), Skeleton::default());
}

#[test]
fn a_parent_comes_first() {
    rejects(
        "/var/state\n/var\n",
        1,
        "parent, /var, isn't a directory listed before it",
    );
    rejects("/run -> var/run\n/run/lock\n", 2, "parent, /run");
}

#[test]
fn paths_are_absolute_canonical_and_outside_usr() {
    rejects("var\n", 1, "not an absolute path");
    rejects("/\n", 1, "the root itself");
    rejects("/var/\n", 1, "canonical");
    rejects("/var//log\n", 1, "canonical");
    rejects("/var\n/var/./log\n", 2, "canonical");
    rejects("/var/..\n", 1, "canonical");
    rejects("/usr\n", 1, "/usr is the packages'");
    rejects("/usr/share\n", 1, "/usr is the packages'");
    assert!(parse("/usrlocal\n").is_ok());
}

#[test]
fn an_entry_is_listed_once() {
    rejects("/var\n/var\n", 2, "listed twice");
    rejects("/lib64 -> a\n/lib64\n", 2, "listed twice");
}

#[test]
fn lines_have_one_of_three_shapes() {
    rejects("/var a b\n", 1, "not `path`");
    rejects("/var a b c\n", 1, "not `path`");
    rejects("/lib64 ->\n", 1, "needs a target");
}

#[test]
fn link_targets_are_relative_and_stay_in_the_root() {
    rejects("/lib64 -> /usr/lib\n", 1, "relative");
    rejects("/lib64 -> ../usr/lib\n", 1, "leaves the root");
    rejects("/lib64 -> usr//lib\n", 1, "empty component");
    assert!(parse("/var\n/var/run -> ../run\n").is_ok());
    rejects("/var\n/var/run -> ../../run\n", 2, "leaves the root");
}

#[test]
fn applying_makes_what_is_missing() {
    let root = tempdir().unwrap();
    let skeleton = parse("/var\n/var/state x\n/lib64 -> usr/lib\n").unwrap();
    let mut asked = Vec::new();
    let applied = apply(root.path(), &skeleton, |at, descriptor| {
        asked.push((at.to_owned(), descriptor.map(str::to_owned)));
        fs::create_dir(at)
    })
    .unwrap();
    assert_eq!(applied.created, ["/var", "/var/state", "/lib64"]);
    assert_eq!(applied.present, 0);
    assert_eq!(
        asked,
        [
            (root.path().join("var"), None),
            (root.path().join("var/state"), Some("x".to_owned())),
        ]
    );
    assert!(root.path().join("var/state").is_dir());
    assert_eq!(
        fs::read_link(root.path().join("lib64")).unwrap(),
        Path::new("usr/lib")
    );
}

#[test]
fn applying_twice_changes_nothing() {
    let root = tempdir().unwrap();
    let skeleton = parse("/var\n/var/state\n/lib64 -> usr/lib\n").unwrap();
    apply(root.path(), &skeleton, plain).unwrap();
    let again = apply(root.path(), &skeleton, |_, _| {
        panic!("made a directory again")
    })
    .unwrap();
    assert!(again.created.is_empty());
    assert_eq!(again.present, 3);
}

#[test]
fn what_is_there_is_left_alone() {
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("var")).unwrap();
    fs::write(root.path().join("var/kept"), "kept").unwrap();
    let skeleton = parse("/var\n/var/log\n").unwrap();
    let applied = apply(root.path(), &skeleton, plain).unwrap();
    assert_eq!(applied.created, ["/var/log"]);
    assert_eq!(
        fs::read_to_string(root.path().join("var/kept")).unwrap(),
        "kept"
    );
}

#[test]
fn something_else_in_the_way_is_an_error() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("tmp"), "").unwrap();
    let error = apply(root.path(), &parse("/tmp\n").unwrap(), plain).unwrap_err();
    assert_eq!(error.path, "/tmp");
    assert!(
        error
            .to_string()
            .contains("a file is in the way of the directory")
    );

    fs::create_dir(root.path().join("lib64")).unwrap();
    let error = apply(root.path(), &parse("/lib64 -> usr/lib\n").unwrap(), plain).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("a directory is in the way of the link")
    );
}

#[test]
fn a_link_in_place_of_a_directory_is_not_followed() {
    let root = tempdir().unwrap();
    let elsewhere = tempdir().unwrap();
    symlink(elsewhere.path(), root.path().join("var")).unwrap();
    let error = apply(root.path(), &parse("/var\n/var/log\n").unwrap(), plain).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("a link is in the way of the directory")
    );
    assert!(!elsewhere.path().join("log").exists());
}

#[test]
fn a_link_to_elsewhere_is_an_error() {
    let root = tempdir().unwrap();
    symlink("elsewhere", root.path().join("lib64")).unwrap();
    let error = apply(root.path(), &parse("/lib64 -> usr/lib\n").unwrap(), plain).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("a link to elsewhere is in the way of the link to usr/lib")
    );
}

#[test]
fn a_failure_stops_where_it_is() {
    let root = tempdir().unwrap();
    let skeleton = parse("/a\n/b\n/c\n").unwrap();
    let error = apply(root.path(), &skeleton, |at, _| {
        if at.ends_with("b") {
            return Err(io::Error::other("refused"));
        }
        fs::create_dir(at)
    })
    .unwrap_err();
    assert_eq!(error.path, "/b");
    assert!(root.path().join("a").is_dir());
    assert!(!root.path().join("c").exists());
}
