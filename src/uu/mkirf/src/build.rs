// spell-checker:ignore (libs) mkirf initramfs cpio
//! The build itself: validate the source layout, walk it into cpio entries,
//! inject the generated `hooks.seq`, and write the deterministic gzip archive
//! through an atomic rename so a failed run never leaves a half-written
//! initramfs behind.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use flate2::{Compression, GzBuilder};

use crate::cpio;
use crate::error::{Error, Result};
use crate::hooks;
use crate::walk::{self, Excludes};

/// Compile the tree at `src` into the cpio.gz at `out`. `src`'s contents map
/// 1:1 onto `/` inside the initramfs, minus any paths matched by `excludes`.
pub fn run(src: &Path, out: &Path, excludes: &Excludes) -> Result<()> {
    let meta = fs::metadata(src).map_err(|e| Error::Layout(format!("{}: {e}", src.display())))?;
    if !meta.is_dir() {
        return Err(Error::Layout(format!("{}: not a directory", src.display())));
    }

    validate_layout(src)?;

    let mut entries = walk::walk(src, excludes).map_err(|e| Error::Io(e.to_string()))?;

    // Resolve the hook DAG and inject the generated hooks.seq manifest as a
    // synthetic entry — it is not a file in the source tree.
    let order = resolve_hooks(src)?;
    entries.push(cpio::Entry {
        name: b"hooks.seq".to_vec(),
        body: cpio::Body::Inline {
            data: hooks::render_seq(&order),
            executable: false,
        },
    });
    // walk() returns sorted entries; the injected one must be re-sorted into
    // archive order (every directory before its descendants).
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    // Write to a sibling temp file and rename into place.
    let tmp = temp_path(out);
    if let Err(e) = write_archive_gz(&tmp, &entries) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::rename(&tmp, out).map_err(|e| Error::Io(format!("{}: {e}", out.display())))?;

    let size = fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    eprintln!(
        "mkirf: wrote {} ({} entries, {size} bytes)",
        out.display(),
        entries.len(),
    );
    Ok(())
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
    if src.join("hooks.seq").exists() {
        return Err(Error::Layout(format!(
            "{}: contains a `hooks.seq`; mkirf generates that file",
            src.display(),
        )));
    }
    Ok(())
}

/// Discover and resolve the hook DAG under `<src>/hooks/`, returning the
/// execution order as cpio-absolute paths. A missing `hooks/` directory means
/// the initramfs has no hooks.
fn resolve_hooks(src: &Path) -> Result<Vec<String>> {
    let hooks_dir = src.join("hooks");
    if !hooks_dir.is_dir() {
        return Ok(Vec::new());
    }
    let hooks = hooks::discover(&hooks_dir).map_err(|e| Error::Hooks(e.to_string()))?;
    let resolved = hooks::resolve(&hooks).map_err(Error::Hooks)?;
    for w in &resolved.warnings {
        eprintln!("mkirf: warning: {w}");
    }
    Ok(resolved.order)
}

/// Compress `entries` into the gzip file at `path`, with mtime 0 and no
/// embedded filename (the `gzip -n` equivalent) so identical input yields
/// byte-identical output. See DESIGN.md §5/§6.
fn write_archive_gz(path: &Path, entries: &[cpio::Entry]) -> Result<()> {
    let file = File::create(path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(BufWriter::new(file), Compression::new(9));
    cpio::write_archive(&mut gz, entries).map_err(|e| Error::Io(e.to_string()))?;
    gz.finish()
        .map_err(|e| Error::Io(e.to_string()))?
        .flush()
        .map_err(|e| Error::Io(e.to_string()))?;
    Ok(())
}

/// A sibling path of `out`, used as the atomic-rename staging file.
fn temp_path(out: &Path) -> PathBuf {
    let mut name = out.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".mkirf-tmp-{}", std::process::id()));
    out.with_file_name(name)
}
