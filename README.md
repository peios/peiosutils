# peiosutils

Peios core userspace utilities. Roughly the equivalent role of `coreutils`
on Linux: the always-installed bin tools every user and admin reaches for.
Each program is a Cargo workspace member building to a static-musl-pie
binary. All members are bundled into a **single** `peiosutils.peipkg`
consumed by `peios-image` (and eventually pkgs.peios.org).

Why one package: at the scale we're at, these tools change together,
ship together, and are universally installed together. Per-tool packaging
buys independent upgrades we don't need yet. If `revstrm` (or any other)
grows enough to warrant its own lifecycle later, it can be split out.

This repo owns the **source** of the tools, not their distribution recipes.
Recipes (versioning, signing) live in `peipkgs-official-recipes` once that
pipeline is wired up. The `dist/` packages produced here are local-dev
artifacts; the farm-built versions are authoritative.

## Layout

```
tools/<category>/<name>/    one tool per directory
crates/<name>/              shared internal libraries (never published)
scripts/                    build + pack helpers
dist/                       .peipkg outputs (gitignored)
```

Categories are organisational, not semantic. Add new ones freely.

## Tools

### `tools/init/`
- `protoinit` — transitional PID 1 stub. Mounts /proc /sys /dev and execs /bin/sh. Will be replaced by real `peinit` when Phase 2 lands.

### `tools/debug/`
- `whoami-token` — dump the calling process's KACS access token.
- `show-sd` — print the KACS security descriptor of one or more paths.
- `revstrm` — KMES event-ring inspector and emitter.

## Building

```
make all          # build every tool's release binary
make pkg          # produce dist/peiosutils_<version>_<arch>.peipkg
make <name>       # build one tool (e.g. make whoami-token)
make clean
```

(`make pkgs` is kept as an alias for `make pkg`.)

`peipkg-build` is built on demand from a sibling clone at `../peipkg-build/`.

## Dependencies

- `../pkm/` (sibling clone) — Cargo `[patch]` redirects `peios-uapi` to its in-tree path. Required for now; once `peios-uapi` ships tagged releases, the patch can be removed.
- `../peipkg-build/` (sibling clone) — Go binary that produces `.peipkg` files. Built into `build/bin/peipkg-build` on first `make pkgs`.

## Adding a new tool

1. `tools/<category>/<name>/{Cargo.toml, src/main.rs, README.md}`
2. Per-tool `Cargo.toml` needs `[package.metadata.peios]` with `install_path` (where the binary lands inside the bundled peipkg — e.g. `bin/foo`).
3. `cargo build -p <name>` from the workspace root picks it up automatically; `make pkg` re-bundles into the single peiosutils peipkg.
4. Add a one-line entry above in the per-category section.

No central registry file. Workspace member globs and `pack-bundle.sh` discover tools by walking `cargo metadata`.
