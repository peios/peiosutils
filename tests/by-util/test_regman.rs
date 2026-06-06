// Integration tests for `regman`, driving the real multicall binary against a
// throwaway corpus. regman reads only on-disk fragments (never the registry),
// so it runs fully in CI with no kernel or LCS.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_peiosutils");

/// The KMES fragment from regman-design.md Appendix A — the canonical example.
const KMES_FRAGMENT: &str = "\
--- machine\\system\\kmes
canonical: Machine\\System\\KMES

The KMES event subsystem reads its operational parameters from the
values under this key. Compiled-in defaults are used at boot.

--- machine\\system\\kmes buffercapacity
canonical: Machine\\System\\KMES BufferCapacity
type: REG_QWORD
default: 4194304
valid: 65536-268435456 bytes (64 KB-256 MB), power of two
applies: live (ring-buffer swap)

Per-CPU ring buffer capacity, in bytes. Must be a power of two; values
that are not are treated as invalid and ignored.

--- machine\\system\\kmes maxeventsize
canonical: Machine\\System\\KMES MaxEventSize
type: REG_DWORD
default: 65536
valid: 1024-4194304 bytes (1 KB-4 MB)
applies: live

Maximum total size of an event emitted from userspace. Oversized events
are rejected with ENOSPC.

--- machine\\system\\kmes maxnestingdepth
canonical: Machine\\System\\KMES MaxNestingDepth
type: REG_DWORD
default: 32
valid: 4-256
applies: live

Maximum msgpack nesting depth permitted in a userspace payload.

--- machine\\system\\kmes maxemitrateperprocess
canonical: Machine\\System\\KMES MaxEmitRatePerProcess
type: REG_DWORD
default: 10000
valid: 100-1000000
applies: live

Maximum events per second a single process may emit from userspace.
";

/// A tempdir corpus with the KMES fragment, plus a never-present default index.
fn corpus() -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("kmes.regman"), KMES_FRAGMENT).unwrap();
    tmp
}

fn regman(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .arg("regman")
        .args(args)
        .env("REGMAN_DIR", dir)
        // Point the index at a path inside the tempdir so tests never touch the
        // real /var/cache and never share state.
        .env("REGMAN_INDEX", dir.join(".index"))
        .env_remove("PAGER")
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn value_lookup_renders_knob_card() {
    let tmp = corpus();
    let out = regman(tmp.path(), &["Machine\\System\\KMES", "BufferCapacity"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("Machine\\System\\KMES BufferCapacity"));
    assert!(s.contains("documented by kmes"));
    assert!(s.contains("Type"));
    assert!(s.contains("REG_QWORD"));
    assert!(s.contains("Applies"));
    assert!(s.contains("Per-CPU ring buffer capacity"));
}

#[test]
fn lookup_is_case_insensitive() {
    let tmp = corpus();
    let out = regman(tmp.path(), &["MACHINE\\system\\KmEs", "buffercapacity"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("Machine\\System\\KMES BufferCapacity"));
}

#[test]
fn forward_slashes_are_normalised() {
    let tmp = corpus();
    let out = regman(tmp.path(), &["Machine/System/KMES", "MaxEventSize"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("Machine\\System\\KMES MaxEventSize"));
}

#[test]
fn key_view_lists_values() {
    let tmp = corpus();
    let out = regman(tmp.path(), &["Machine\\System\\KMES"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("Values"));
    assert!(s.contains("BufferCapacity"));
    assert!(s.contains("MaxEventSize"));
    assert!(s.contains("MaxNestingDepth"));
    assert!(s.contains("MaxEmitRatePerProcess"));
    // The one-line summary comes from each value's first body line.
    assert!(s.contains("Per-CPU ring buffer capacity, in bytes."));
}

#[test]
fn unknown_path_exits_2() {
    let tmp = corpus();
    let out = regman(tmp.path(), &["Machine\\System\\Nope", "Nothing"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn works_with_a_built_index() {
    let tmp = corpus();
    let build = regman(tmp.path(), &["index"]);
    assert!(build.status.success());
    assert!(tmp.path().join(".index").exists());

    // Lookup should now go through the index (tier 1) and return the same thing.
    let out = regman(tmp.path(), &["Machine\\System\\KMES", "BufferCapacity"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("Machine\\System\\KMES BufferCapacity"));

    let clear = regman(tmp.path(), &["index", "clear"]);
    assert!(clear.status.success());
    assert!(!tmp.path().join(".index").exists());
}

#[test]
fn markdown_is_rendered_not_literal() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("x.regman"),
        "--- a\\b foo\ncanonical: A\\B Foo\ntype: REG_DWORD\n\nThis is **bold** and has `code`.\n",
    )
    .unwrap();
    let out = regman(tmp.path(), &["A\\B", "Foo"]);
    assert!(out.status.success());
    let s = stdout(&out);
    // Piped output carries no ANSI and no literal Markdown markers.
    assert!(s.contains("This is bold and has code."));
    assert!(!s.contains("**"));
    assert!(!s.contains('`'));
    assert!(!s.contains('\u{1b}'));
}

#[test]
fn apropos_searches_names_and_summaries() {
    let tmp = corpus();
    let out = regman(tmp.path(), &["-k", "rate"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("Machine\\System\\KMES MaxEmitRatePerProcess"));
    // A term that matches nothing exits 2.
    let miss = regman(tmp.path(), &["-k", "thismatchesnothing"]);
    assert_eq!(miss.status.code(), Some(2));
}

#[test]
fn apropos_and_terms_narrow() {
    let tmp = corpus();
    // "max" matches several values; adding "nesting" narrows to MaxNestingDepth
    // (only its name/summary contain "nesting").
    let out = regman(tmp.path(), &["-k", "max", "nesting"]);
    assert!(out.status.success());
    let s = stdout(&out);
    assert!(s.contains("MaxNestingDepth"));
    assert!(!s.contains("MaxEventSize"));
    assert!(!s.contains("MaxEmitRatePerProcess"));
}

#[test]
fn lint_accepts_the_kmes_fragment() {
    let tmp = corpus();
    let out = regman(tmp.path(), &["lint", tmp.path().join("kmes.regman").to_str().unwrap()]);
    assert!(out.status.success(), "lint stderr: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn lint_rejects_a_bad_anchor() {
    let tmp = corpus();
    let bad = tmp.path().join("bad.regman");
    std::fs::write(
        &bad,
        "--- wrong anchor\ncanonical: Machine\\System\\X Foo\ntype: REG_DWORD\n\nbody\n",
    )
    .unwrap();
    let out = regman(tmp.path(), &["lint", bad.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn fmt_bakes_the_anchor() {
    let tmp = corpus();
    let frag = tmp.path().join("bad.regman");
    std::fs::write(
        &frag,
        "--- wrong\ncanonical: Machine\\System\\X Foo\ntype: REG_DWORD\n\nbody\n",
    )
    .unwrap();
    let out = regman(tmp.path(), &["fmt", frag.to_str().unwrap()]);
    assert!(out.status.success());
    let fixed = std::fs::read_to_string(&frag).unwrap();
    assert!(fixed.starts_with("--- machine\\system\\x foo\n"));
    // And lint now passes.
    let lint = regman(tmp.path(), &["lint", frag.to_str().unwrap()]);
    assert!(lint.status.success());
}
