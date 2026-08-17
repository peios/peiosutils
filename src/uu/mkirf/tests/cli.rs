//! End-to-end tests exercising the real `mkirf` binary.

use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use flate2::read::GzDecoder;
use tempfile::tempdir;

/// Path to the `mkirf` binary, provided by Cargo to integration tests.
const MKIRF: &str = env!("CARGO_BIN_EXE_mkirf");

fn sample_tree(root: &Path) {
    fs::create_dir_all(root.join("usr/bin")).unwrap();
    fs::write(root.join("init"), b"#!/bin/sh\n").unwrap();
    fs::set_permissions(root.join("init"), PermissionsExt::from_mode(0o755)).unwrap();
    fs::write(root.join("usr/bin/tool"), b"binary contents").unwrap();
}

#[test]
fn produces_a_gzip_archive() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    sample_tree(&src);
    let out = dir.path().join("initramfs.cpio.gz");

    let status = Command::new(MKIRF).arg(&src).arg(&out).status().unwrap();
    assert!(status.success());

    let bytes = fs::read(&out).unwrap();
    assert_eq!(&bytes[..2], &[0x1f, 0x8b], "gzip magic");
}

#[test]
fn output_is_byte_identical_across_runs() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    sample_tree(&src);

    let a = dir.path().join("a.cpio.gz");
    let b = dir.path().join("b.cpio.gz");
    assert!(
        Command::new(MKIRF)
            .arg(&src)
            .arg(&a)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(MKIRF)
            .arg(&src)
            .arg(&b)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
}

#[test]
fn rejects_a_missing_source() {
    let dir = tempdir().unwrap();
    let status = Command::new(MKIRF)
        .arg(dir.path().join("does-not-exist"))
        .arg(dir.path().join("out.cpio.gz"))
        .status()
        .unwrap();
    assert!(!status.success());
}

#[test]
fn rejects_a_wrong_argument_count() {
    assert!(!Command::new(MKIRF).status().unwrap().success());
}

#[test]
fn archive_extracts_with_gnu_cpio() {
    // An independent check against a real cpio reader. Skipped where the
    // `cpio` tool is not installed.
    if Command::new("cpio").arg("--version").output().is_err() {
        eprintln!("skipping archive_extracts_with_gnu_cpio: `cpio` not found");
        return;
    }

    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    sample_tree(&src);
    fs::set_permissions(src.join("usr/bin/tool"), PermissionsExt::from_mode(0o755)).unwrap();

    let out = dir.path().join("initramfs.cpio.gz");
    assert!(
        Command::new(MKIRF)
            .arg(&src)
            .arg(&out)
            .status()
            .unwrap()
            .success()
    );

    let dest = dir.path().join("extracted");
    fs::create_dir(&dest).unwrap();
    let extracted = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "gzip -dc '{}' | cpio -idm -D '{}'",
            out.display(),
            dest.display(),
        ))
        .status()
        .unwrap();
    assert!(
        extracted.success(),
        "GNU cpio failed to extract the archive"
    );

    assert_eq!(fs::read(dest.join("init")).unwrap(), b"#!/bin/sh\n");
    assert_eq!(
        fs::read(dest.join("usr/bin/tool")).unwrap(),
        b"binary contents"
    );
    let mode = fs::metadata(dest.join("usr/bin/tool"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(
        mode & 0o111,
        0o111,
        "executable bit survived the round trip"
    );
}

/// Decompress a gzip blob to its raw bytes.
fn gunzip(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut out).unwrap();
    out
}

#[test]
fn hooks_seq_lists_hooks_in_dependency_order() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    sample_tree(&src);

    // `a-consumer` requires the capability `z-producer` provides, so the
    // resolved order must place the producer first — overriding the name
    // order, which would otherwise put `a-consumer` first.
    let hooks = src.join("hooks");
    fs::create_dir(&hooks).unwrap();
    fs::write(
        hooks.join("z-producer.sh"),
        "#!/bin/sh\n# /// hook\n# provides = [\"ready\"]\n# ///\n",
    )
    .unwrap();
    fs::write(
        hooks.join("a-consumer.sh"),
        "#!/bin/sh\n# /// hook\n# requires = [\"ready\"]\n# ///\n",
    )
    .unwrap();

    let out = dir.path().join("initramfs.cpio.gz");
    assert!(
        Command::new(MKIRF)
            .arg(&src)
            .arg(&out)
            .status()
            .unwrap()
            .success()
    );

    // The sequences are stored uncompressed within the cpio; find their
    // bodies in the decompressed archive.
    let archive = gunzip(&fs::read(&out).unwrap());

    // Both the entry NAME and the body are asserted. Checking only the body
    // let an earlier version of this test keep passing when the file moved
    // out of the archive root, which is precisely what it existed to catch.
    for name in [
        b"system/prelude/hooks.seq.1".as_slice(),
        b"system/prelude/hooks.seq.2".as_slice(),
    ] {
        assert!(
            contains(&archive, name),
            "no cpio entry named {}",
            String::from_utf8_lossy(name),
        );
    }

    let v1 = b"hookseq 1\n/hooks/z-producer.sh\n/hooks/a-consumer.sh\n";
    assert!(
        contains(&archive, v1),
        "hooks.seq.1 not present in dependency order",
    );

    // Version 2 carries the declarations that produced that order, with the
    // stanzas in the same resolved sequence.
    let v2 = b"hookseq 2\n\
               hook /hooks/z-producer.sh\nprovides ready\n\
               hook /hooks/a-consumer.sh\nrequires ready\n";
    assert!(
        contains(&archive, v2),
        "hooks.seq.2 not present, or not in the expected shape",
    );
}

/// Whether `haystack` contains `needle` as a contiguous byte run.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn a_hookless_image_still_gets_both_sequences() {
    // Emission is unconditional so that prelude can treat a MISSING sequence
    // as an error. An image with no hooks gets a marker line and no entries,
    // not an absent file.
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    sample_tree(&src);

    let out = dir.path().join("initramfs.cpio.gz");
    assert!(
        Command::new(MKIRF)
            .arg(&src)
            .arg(&out)
            .status()
            .unwrap()
            .success()
    );

    let archive = gunzip(&fs::read(&out).unwrap());
    assert!(contains(&archive, b"system/prelude/hooks.seq.1"));
    assert!(contains(&archive, b"system/prelude/hooks.seq.2"));
    assert!(contains(&archive, b"hookseq 1\n"));
    assert!(contains(&archive, b"hookseq 2\n"));
}

#[test]
fn the_sequence_directory_is_created_when_the_source_lacks_it() {
    // The sequences are injected rather than walked, so their parent
    // directories may not exist in the source tree. A cpio whose file
    // entries have no parent directory entries does not unpack.
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    sample_tree(&src);
    assert!(!src.join("system").exists(), "test premise: no system/ in src");

    let out = dir.path().join("initramfs.cpio.gz");
    assert!(
        Command::new(MKIRF)
            .arg(&src)
            .arg(&out)
            .status()
            .unwrap()
            .success()
    );

    let archive = gunzip(&fs::read(&out).unwrap());
    assert!(contains(&archive, b"system\0"), "no `system` directory entry");
    assert!(
        contains(&archive, b"system/prelude\0"),
        "no `system/prelude` directory entry",
    );
}

#[test]
fn rejects_a_source_without_init() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("not-init"), b"x").unwrap();

    let status = Command::new(MKIRF)
        .arg(&src)
        .arg(dir.path().join("out.cpio.gz"))
        .status()
        .unwrap();
    assert!(
        !status.success(),
        "a source with no `init` must be rejected"
    );
}

/// A newc entry recovered from a raw (uncompressed) cpio stream: its name,
/// the absolute offset of its file data, the data length, and the st_mode.
struct NewcEntry {
    name: Vec<u8>,
    data_off: usize,
    size: usize,
    mode: u32,
}

/// Walk the uncompressed newc cpio at the front of `buf`, returning its
/// entries and the offset just past its `TRAILER!!!` — where the gzip main
/// archive begins.
fn walk_early(buf: &[u8]) -> (Vec<NewcEntry>, usize) {
    let roundup4 = |n: usize| (n + 3) & !3;
    let mut entries = Vec::new();
    let mut pos = 0usize;
    loop {
        assert_eq!(&buf[pos..pos + 6], b"070701", "newc magic at {pos}");
        let field = |i: usize| -> usize {
            let raw = &buf[pos + 6 + i * 8..pos + 6 + i * 8 + 8];
            usize::from_str_radix(std::str::from_utf8(raw).unwrap(), 16).unwrap()
        };
        let mode = field(1) as u32;
        let size = field(6);
        let namesize = field(11);
        // The name field may be NUL-widened for 16-byte data alignment; the
        // real path is the bytes up to the first NUL.
        let raw = &buf[pos + 110..pos + 110 + namesize - 1];
        let name = raw.split(|&b| b == 0).next().unwrap().to_vec();
        let data_off = pos + roundup4(110 + namesize);
        let is_trailer = name.starts_with(b"TRAILER!!!");
        pos = data_off + roundup4(size);
        if is_trailer {
            break;
        }
        entries.push(NewcEntry {
            name,
            data_off,
            size,
            mode,
        });
    }
    (entries, pos)
}

#[test]
fn early_segments_become_an_aligned_uncompressed_prefix() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    sample_tree(&src);

    // Two early segments that share the `kernel/` parent — microcode and an
    // ACPI table override. The segment dir name (`microcode`/`acpi`) is a
    // label and must not appear in the cpio paths.
    let blob = vec![0xABu8; 4099]; // odd size, so data alignment is non-trivial
    let uc = src.join("++/microcode/kernel/x86/microcode");
    fs::create_dir_all(&uc).unwrap();
    fs::write(uc.join("GenuineIntel.bin"), &blob).unwrap();
    let acpi = src.join("++/acpi/kernel/firmware/acpi");
    fs::create_dir_all(&acpi).unwrap();
    fs::write(acpi.join("ssdt.aml"), b"FAKEAML").unwrap();

    let out = dir.path().join("initramfs.cpio.gz");
    assert!(
        Command::new(MKIRF)
            .arg(&src)
            .arg(&out)
            .status()
            .unwrap()
            .success()
    );
    let bytes = fs::read(&out).unwrap();

    // The image opens with an UNCOMPRESSED cpio, not gzip — that is what the
    // kernel's early scanner reads before any decompression.
    assert_eq!(&bytes[..6], b"070701", "early region must be uncompressed cpio");

    let (early, gz_start) = walk_early(&bytes);
    let find = |p: &[u8]| early.iter().find(|e| e.name == p);

    // The microcode blob is present at its kernel-ABI path, 16-byte aligned,
    // with its bytes intact — the segment label `microcode/` is stripped.
    let mc = find(b"kernel/x86/microcode/GenuineIntel.bin").expect("microcode entry");
    assert_eq!(mc.mode & 0o170_000, 0o100_000, "regular file");
    assert_eq!(mc.data_off % 16, 0, "microcode payload must be 16-byte aligned");
    assert_eq!(&bytes[mc.data_off..mc.data_off + mc.size], &blob[..]);

    // The ACPI segment rides the same region, also aligned.
    let aml = find(b"kernel/firmware/acpi/ssdt.aml").expect("acpi entry");
    assert_eq!(aml.data_off % 16, 0, "acpi payload must be 16-byte aligned");

    // The shared `kernel/` parent appears exactly once despite two segments.
    assert_eq!(
        early.iter().filter(|e| e.name == b"kernel").count(),
        1,
        "shared parent dir must be de-duplicated",
    );

    // The gzip main archive follows, and it does NOT carry the `++` tree.
    assert_eq!(&bytes[gz_start..gz_start + 2], &[0x1f, 0x8b], "gzip after early region");
    let main = gunzip(&bytes[gz_start..]);
    assert!(
        main.windows(4).any(|w| w == b"init"),
        "main archive still carries the real tree",
    );
    assert!(
        !main.windows(2).any(|w| w == b"++"),
        "the ++ tree must not leak into the compressed main archive",
    );
    assert!(
        !main
            .windows(b"GenuineIntel.bin".len())
            .any(|w| w == b"GenuineIntel.bin"),
        "microcode must live only in the early region, not the main archive",
    );
}

#[test]
fn no_early_segments_is_a_plain_gzip() {
    // Without a `++/` tree the output is byte-for-byte the v1 single gzip.
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    sample_tree(&src);
    let out = dir.path().join("initramfs.cpio.gz");
    assert!(
        Command::new(MKIRF)
            .arg(&src)
            .arg(&out)
            .status()
            .unwrap()
            .success()
    );
    let bytes = fs::read(&out).unwrap();
    assert_eq!(&bytes[..2], &[0x1f, 0x8b], "no early region → leading gzip");
}

/// Poll `cond` until it returns true or `timeout` elapses.
fn wait_for(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn watch_rebuilds_when_the_source_changes() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    sample_tree(&src);
    let out = dir.path().join("initramfs.cpio.gz");

    let mut child = Command::new(MKIRF)
        .args(["--watch", "--debounce", "1"])
        .arg(&src)
        .arg(&out)
        .spawn()
        .unwrap();

    // Watch mode builds once on startup.
    let started = wait_for(Duration::from_secs(20), || out.exists());
    let first = started.then(|| fs::read(&out).unwrap());

    // A change to the source tree must trigger a rebuild.
    fs::write(src.join("added-file"), b"new content").unwrap();
    let rebuilt = match &first {
        Some(first) => wait_for(Duration::from_secs(20), || {
            fs::read(&out).map(|now| &now != first).unwrap_or(false)
        }),
        None => false,
    };

    let _ = child.kill();
    let _ = child.wait();

    assert!(started, "watch did not produce an initial build");
    assert!(rebuilt, "watch did not rebuild after a source change");
}
