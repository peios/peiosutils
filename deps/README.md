# `deps/` — vendored libpeios (temporary build bridge)

The Peios-native tools (token, sd, logonse, revstrm, and the SD-aware file
utilities via uucore's `sd-control`/`preserve`) link **libpeios** through the
`peios` crate. Building them therefore needs libpeios's headers (to bindgen)
and its shared object (to link).

Until the build root can provide libpeios as a package (glibc + libpeios live
in `pkgs/`; the runtime/`-devel` packaging is libpeios's pekit recipe), this
directory **vendors** those artifacts so a plain `cargo build` works here:

```
deps/
  include/peios.h, include/peios/*.h   libpeios public headers
  include/pkm/*.h                       pkm UAPI headers the peios headers #include
  lib/libpeios.so.0                     runtime shared object (SONAME libpeios.so.0)
  lib/libpeios.so -> libpeios.so.0      dev symlink for `-lpeios`
  lib/libpeios.a                        static archive (unused by these std binaries)
```

`.cargo/config.toml` points `peios-sys` at this directory via
`PEIOS_LIB_DIR`/`PEIOS_INCLUDE`/`PKM_UAPI` (relative to the workspace root), so
no per-invocation env is needed for the libpeios side. bindgen still needs
`libclang` on the host (`LIBCLANG_PATH`), which is environment-specific.

The produced binaries carry a normal `DT_NEEDED libpeios.so.0` and **no rpath**
— they resolve libpeios from the system path at runtime (what the packaged
build wants). To run them against this vendored copy locally, set
`LD_LIBRARY_PATH=deps/lib`.

## Running the test suite locally

Two per-invocation variables, neither of which can live in
`.cargo/config.toml` — cargo's `[env]` does not reach the binary cargo spawns,
and the bindgen one has to be computed:

```sh
export LD_LIBRARY_PATH="$PWD/deps/lib"
export BINDGEN_EXTRA_CLANG_ARGS="-isystem $(gcc -print-file-name=include)"
cargo test --locked --no-default-features --features feat_os_unix
```

`LD_LIBRARY_PATH` is the no-rpath consequence above. The test harness clears
the environment before spawning the binary and forwards this variable
explicitly (`tests/uutests/src/lib/util.rs`) — without that forwarding every
test that runs the binary fails with a loader error rather than an assertion,
which reads like real test signal and is not.

`BINDGEN_EXTRA_CLANG_ARGS` is needed when the host has libclang but not
clang's own builtin headers — bindgen then cannot find `stddef.h` and dies
against `<peios.h>`. On Debian/Ubuntu that means `libclang1-*` is installed but
`libclang-common-*-dev` is not; installing the latter (or `clang`) fixes it
properly and makes the variable unnecessary. `pekit.toml` exports the same
workaround for the packaged build.

## Provenance / refreshing

The artifacts are **git-ignored** (regenerable); this README and
`refresh.sh` are tracked. Vendored from **libpeios v0.3.2** (built against
kernel/pkm v0.20.1-rc2, rev a47c638). Regenerate after a libpeios change:

```sh
LIBPEIOS=../libpeios PKM=../pkm ./deps/refresh.sh
```

This is a stopgap: once the build root installs `libpeios-devel` +
`kernel-headers`, drop `deps/` and the `.cargo/config.toml` `[env]` overrides —
`peios-sys` resolves libpeios via `pkg-config peios` automatically.
