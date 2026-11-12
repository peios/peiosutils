// spell-checker:ignore (libs) mkirf initramfs cpio zstd
//! The build itself: validate the source layout, walk it into cpio entries,
//! inject the generated `hooks.seq`, and write the deterministic compressed
//! archive through an atomic rename so a failed run never leaves a
//! half-written initramfs behind.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use flate2::{Compression, GzBuilder};

use crate::cli::Compress;
use crate::cpio;
use crate::error::{Error, Result};
use crate::hooks;
use crate::walk::{self, Excludes};

/// Compile the tree at `src` into the compressed cpio at `out`. `src`'s
/// contents map 1:1 onto `/` inside the initramfs, minus any paths matched
/// by `excludes`.
pub fn run(src: &Path, out: &Path, excludes: &Excludes, compress: Compress) -> Result<()> {
    let meta = fs::metadata(src).map_err(|e| Error::Layout(format!("{}: {e}", src.display())))?;
    if !meta.is_dir() {
        return Err(Error::Layout(format!("{}: not a directory", src.display())));
    }

    validate_layout(src)?;

    let mut entries = walk::walk(src, excludes).map_err(|e| Error::Io(e.to_string()))?;
    // The early directories are not part of the main (compressed) archive —
    // they become the uncompressed early region prepended ahead of it (see
    // collect_early).
    entries.retain(|e| !is_early(&e.name));

    // Resolve the hook DAG and inject the generated sequence manifests as
    // synthetic entries — they are not files in the source tree.
    //
    // Every version mkirf can project faithfully is emitted, because the
    // writer (this, in peiosutils) and the reader (prelude) ship in two
    // independently versioned packages: an image may pair a new mkirf with
    // an old prelude, and the version lives in the FILE NAME precisely so
    // one image can satisfy both. Version 1 stays honest for free — a flat
    // order is derivable from the DAG — and gets dropped on the day some
    // future format carries something it cannot express.
    //
    // Emitted unconditionally, including for a hookless image, where they
    // are a marker line and nothing else. That is what lets prelude treat a
    // MISSING sequence as an error rather than having to decide whether the
    // absence means "no hooks" or "the file I needed is not here".
    let (hooks, order) = resolve_hooks(src)?;
    for (version, data) in [
        (1, hooks::render_seq(&order)),
        (2, hooks::render_seq_v2(&hooks, &order)),
    ] {
        entries.push(cpio::Entry {
            name: format!("{SEQ_DIR}/hooks.seq.{version}").into_bytes(),
            body: cpio::Body::Inline {
                data,
                executable: false,
            },
        });
    }
    add_missing_ancestors(&mut entries, SEQ_DIR);
    // walk() returns sorted entries; the injected ones must be re-sorted into
    // archive order (every directory before its descendants).
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    let (early, early_warnings) = collect_early(src)?;
    for w in &early_warnings {
        eprintln!("mkirf: warning: {w}");
    }

    // Write to a sibling temp file and rename into place.
    let tmp = temp_path(out);
    if let Err(e) = write_image(&tmp, &early, &entries, compress) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::rename(&tmp, out).map_err(|e| Error::Io(format!("{}: {e}", out.display())))?;

    let size = fs::metadata(out).map_or(0, |m| m.len());
    if early.is_empty() {
        eprintln!(
            "mkirf: wrote {} ({} entries, {size} bytes)",
            out.display(),
            entries.len(),
        );
    } else {
        eprintln!(
            "mkirf: wrote {} ({} entries + {} early, {size} bytes)",
            out.display(),
            entries.len(),
            early.len(),
        );
    }
    Ok(())
}

/// Where the generated hook sequences are placed inside the initramfs.
///
/// The `/system` tier, not `/usr` and not `/var/state`: these files are
/// DERIVED — mkirf generates them per image from the hooks directory — which
/// is what `/system` means in the layout (`system/retc` holds reconciler
/// output the same way). `/usr` is the vendor tier and no package ships
/// these; `/var/state` is the mutable tier and these are immutable build
/// output.
const SEQ_DIR: &str = "system/prelude";

/// Insert directory entries for every ancestor of `dir` that `entries` does
/// not already contain.
///
/// The sequence files are injected rather than walked, so their parent
/// directories may not exist in the source tree at all — and a cpio whose
/// file entries have no parent directory entries does not unpack. mkirf
/// synthesises them rather than requiring the source tree to provide them,
/// so the output does not depend on which base-filesystem package the image
/// happened to compose.
fn add_missing_ancestors(entries: &mut Vec<cpio::Entry>, dir: &str) {
    let mut prefix = String::new();
    for component in dir.split('/') {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        let name = prefix.as_bytes();
        if !entries.iter().any(|e| e.name == name) {
            entries.push(cpio::Entry {
                name: name.to_vec(),
                body: cpio::Body::Directory,
            });
        }
    }
}

/// The directories holding the early region's segments, in precedence
/// order, as [`hooks::HOOK_DIRS`] holds the hooks: `/usr` is what packages
/// ship and `/lcl` what the operator puts there by hand. A segment in `/lcl`
/// replaces the one of the same name in `/usr`, so a package's microcode can
/// be overridden without deleting its payload.
///
/// The legacy `++/` at the archive root, from before packages could ship
/// only `/usr`, is read last and warned about.
const EARLY_DIRS: &[&str] = &["lcl/lib/earlyirf", "usr/lib/earlyirf", LEGACY_EARLY_DIR];

/// The one in [`EARLY_DIRS`] that is on its way out.
const LEGACY_EARLY_DIR: &str = "++";

/// True if `name` is one of [`EARLY_DIRS`] or a path inside one: the early
/// region's, not the main archive's.
fn is_early(name: &[u8]) -> bool {
    EARLY_DIRS.iter().any(|dir| {
        let dir = dir.as_bytes();
        name == dir || (name.starts_with(dir) && name.get(dir.len()) == Some(&b'/'))
    })
}

/// Collect the early region's segments from [`EARLY_DIRS`], with any
/// advisories.
///
/// The early region is the content the kernel consumes *before* it
/// decompresses the main initramfs: CPU microcode and ACPI table overrides.
/// Each immediate child of an early directory is one segment whose own
/// contents map onto the cpio root, so
/// `usr/lib/earlyirf/intel-ucode/kernel/x86/microcode/GenuineIntel.bin`
/// becomes `kernel/x86/microcode/GenuineIntel.bin`. The segment directory
/// name (`intel-ucode`, `acpi`, …) is just a label and never appears in the
/// archive. A name identifies a segment, so the same name in two directories
/// is one segment, and the earlier directory's wins. All segments merge into
/// one uncompressed cpio prepended ahead of the main archive. See DESIGN.md
/// §10.
///
/// mkirf stays format-agnostic: it knows "early segments", never "microcode".
fn collect_early(src: &Path) -> Result<(Vec<cpio::Entry>, Vec<String>)> {
    let no_excludes = Excludes::compile(&[]).map_err(|e| Error::Io(e.to_string()))?;
    let mut entries: Vec<cpio::Entry> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    // Each segment taken, by name, with where it was taken from.
    let mut taken: Vec<(std::ffi::OsString, String)> = Vec::new();

    for dest in EARLY_DIRS {
        let dir = src.join(dest);
        let Ok(meta) = fs::symlink_metadata(&dir) else {
            continue;
        };
        if !meta.is_dir() {
            return Err(Error::Layout(format!(
                "{}: `{dest}` must be a directory of early segments",
                src.display(),
            )));
        }

        // Sort segment directories for deterministic merge order.
        let mut children: Vec<PathBuf> = fs::read_dir(&dir)
            .map_err(|e| Error::Io(format!("{}: {e}", dir.display())))?
            .map(|d| d.map(|d| d.path()))
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| Error::Io(format!("{}: {e}", dir.display())))?;
        children.sort();

        for child in children {
            let name = child.file_name().unwrap_or_default().to_owned();
            let label = name.to_string_lossy().into_owned();
            let cmeta = fs::symlink_metadata(&child)
                .map_err(|e| Error::Io(format!("{}: {e}", child.display())))?;
            if !cmeta.is_dir() {
                return Err(Error::Layout(format!(
                    "{}: every entry of `{dest}` must be a directory (one per early segment) — \
                     `{label}` is not. An early segment is a tree (e.g. \
                     intel-ucode/kernel/x86/microcode/), not a file.",
                    src.display(),
                )));
            }
            if let Some((_, winner)) = taken.iter().find(|(n, _)| *n == name) {
                warnings.push(format!(
                    "early segment `{label}` in /{dest} is shadowed by /{winner}/{label}"
                ));
                continue;
            }
            if *dest == LEGACY_EARLY_DIR {
                warnings.push(format!(
                    "early segment `{label}` is in the legacy /{LEGACY_EARLY_DIR} directory; \
                     move it to /usr/lib/earlyirf (packaged) or /lcl/lib/earlyirf (local)"
                ));
            }
            let seg = walk::walk(&child, &no_excludes).map_err(|e| Error::Io(e.to_string()))?;
            entries.extend(seg);
            taken.push((name, (*dest).to_owned()));
        }
    }

    // Segments share parent directories (e.g. both microcode and acpi root at
    // `kernel/`). Merge into archive order, then drop duplicate directory
    // entries; a genuine file collision between segments is an error.
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((dedup_segment_dirs(entries)?, warnings))
}

/// Deduplicate the shared parent directories produced when several early
/// segments are merged. Two identical directory entries collapse to one; two
/// entries with the same name where either is not a directory is a real
/// conflict between segments and fails the build. Input must be sorted.
fn dedup_segment_dirs(entries: Vec<cpio::Entry>) -> Result<Vec<cpio::Entry>> {
    let mut out: Vec<cpio::Entry> = Vec::with_capacity(entries.len());
    for e in entries {
        if let Some(prev) = out.last() {
            if prev.name == e.name {
                let both_dirs = matches!(prev.body, cpio::Body::Directory)
                    && matches!(e.body, cpio::Body::Directory);
                if both_dirs {
                    continue; // shared parent dir — keep one
                }
                return Err(Error::Layout(format!(
                    "early segments both define `{}`",
                    String::from_utf8_lossy(&e.name),
                )));
            }
        }
        out.push(e);
    }
    Ok(out)
}

/// Check the source tree is a layout prelude can boot.
///
/// It must carry an executable `init` (the initramfs PID 1) — a symlink to the
/// real binary is fine, as long as it resolves within the tree — and must not
/// already contain a `hooks.seq`, which mkirf generates.
fn validate_layout(src: &Path) -> Result<()> {
    let init = src.join("init");
    match fs::metadata(&init) {
        Ok(m) if m.is_file() => {
            if m.mode() & 0o111 == 0 {
                return Err(Error::Layout(format!(
                    "{}: `init` is not executable",
                    src.display()
                )));
            }
        }
        Ok(_) => {
            return Err(Error::Layout(format!(
                "{}: `init` is not a regular file",
                src.display()
            )));
        }
        Err(_) => {
            return Err(Error::Layout(format!(
                "{}: no `init` — an initramfs needs a PID 1",
                src.display()
            )));
        }
    }
    // `hooks.seq` at the root is the legacy name mkirf used to generate. It
    // is no longer written, but it is still rejected here: prelude reads it
    // as a fallback, so a stray one in the source tree would be picked up as
    // a sequence nothing generated.
    for reserved in [
        "hooks.seq",
        &format!("{SEQ_DIR}/hooks.seq.1"),
        &format!("{SEQ_DIR}/hooks.seq.2"),
    ] {
        if src.join(reserved).exists() {
            return Err(Error::Layout(format!(
                "{}: contains a `{reserved}`; mkirf generates that file",
                src.display(),
            )));
        }
    }
    Ok(())
}

/// Discover and resolve the hook DAG under `<src>/hooks/`, returning the
/// hooks and their execution order as cpio-absolute paths. A missing
/// `hooks/` directory means the initramfs has no hooks — which is a valid
/// image, and produces empty sequence files rather than none.
fn resolve_hooks(src: &Path) -> Result<(Vec<hooks::Hook>, Vec<String>)> {
    let (mut hooks, discovery_warnings) =
        hooks::discover_all(src).map_err(|e| Error::Hooks(e.to_string()))?;
    for w in &discovery_warnings {
        eprintln!("mkirf: warning: {w}");
    }
    if hooks.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    // Before resolving: give every hook that is not part of `initramfs-ready`
    // the implicit edge that orders it after it. Done here rather than inside
    // resolve() so the edge is also what gets written into hooks.seq.2 — the
    // rule lives in one place and the sequence explains itself.
    hooks::apply_implicit_ordering(&mut hooks);
    let resolved = hooks::resolve(&hooks).map_err(Error::Hooks)?;
    for w in &resolved.warnings {
        eprintln!("mkirf: warning: {w}");
    }
    Ok((hooks, resolved.order))
}

/// Write the initramfs image at `path`: the `early` segments as a leading
/// **uncompressed** cpio (the kernel scans it before decompression — §10),
/// immediately followed by the compressed `main` archive on the same
/// stream. With no early segments this is exactly a single compressed
/// member.
///
/// Both compressors are deterministic at their fixed level, the gzip
/// member carries mtime 0 and no embedded filename (the `gzip -n`
/// equivalent), and the early region is byte-stable, so identical input
/// yields byte-identical output. See DESIGN.md §5/§6/§10.
fn write_image(
    path: &Path,
    early: &[cpio::Entry],
    main: &[cpio::Entry],
    compress: Compress,
) -> Result<()> {
    let file = File::create(path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    let mut bw = BufWriter::new(file);

    if !early.is_empty() {
        cpio::write_early_archive(&mut bw, early).map_err(|e| Error::Io(e.to_string()))?;
    }

    match compress {
        // Level 3, single-threaded: measured on a real image it beats
        // gzip level 6 on both axes (~9× the compression speed, slightly
        // smaller output), and single-threading keeps the bytes
        // independent of the build host's core count.
        Compress::Zstd => {
            let mut zs =
                zstd::stream::write::Encoder::new(bw, 3).map_err(|e| Error::Io(e.to_string()))?;
            cpio::write_archive(&mut zs, main).map_err(|e| Error::Io(e.to_string()))?;
            zs.finish()
                .map_err(|e| Error::Io(e.to_string()))?
                .flush()
                .map_err(|e| Error::Io(e.to_string()))?;
        }
        // Level 6, not 9: ~0.16% of output size buys more than twice the
        // compression time, and mkirf sits on every rebuild's critical
        // path.
        Compress::Gzip => {
            let mut gz = GzBuilder::new().mtime(0).write(bw, Compression::new(6));
            cpio::write_archive(&mut gz, main).map_err(|e| Error::Io(e.to_string()))?;
            gz.finish()
                .map_err(|e| Error::Io(e.to_string()))?
                .flush()
                .map_err(|e| Error::Io(e.to_string()))?;
        }
    }
    Ok(())
}

/// A sibling path of `out`, used as the atomic-rename staging file.
fn temp_path(out: &Path) -> PathBuf {
    let mut name = out.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".mkirf-tmp-{}", std::process::id()));
    out.with_file_name(name)
}
