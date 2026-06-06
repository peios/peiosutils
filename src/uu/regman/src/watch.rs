// `regman index --watch` — stay resident and rebuild the index on change.
//
// This is the freshness mechanism (design §7.4): run as a peinit-supervised
// service (like `mkirf --watch`), it keeps the fast path warm. Correctness
// never depends on it — the cascade tolerates a stale or absent index — so a
// missed event only ever costs speed.

use std::path::Path;
use std::sync::mpsc;

use notify::{RecursiveMode, Watcher};

use crate::error::{Error, Result};
use crate::index;

pub fn run(dir: &Path, index_path: &Path) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .map_err(|e| Error::Io(e.to_string()))?;
    watcher
        .watch(dir, RecursiveMode::NonRecursive)
        .map_err(|e| Error::Io(e.to_string()))?;

    while let Ok(_event) = rx.recv() {
        // Coalesce a burst (a package install touches several files at once),
        // then rebuild once.
        while rx.try_recv().is_ok() {}
        index::build(dir, index_path)?;
    }
    Ok(())
}
