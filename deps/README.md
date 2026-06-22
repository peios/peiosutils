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
