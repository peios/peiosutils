# Peiosutils

Peiosutils provides command-line utilities for files, text, accounts, security,
registry data, disks, and system administration. It is a maintained hard fork
of uutils/coreutils for Peios. Its identity and permission commands use the
Peios security model.

The `peiosutils` executable dispatches by its first argument or by the name
of an applet symlink. Applet implementations live under `src/uu/`; shared
internal code lives in `src/uucore/` and `crates/`.

## Build and check

Bazel compiles native Rust targets and assembles PPK blobs and manifests.
Prepare a development SDK as described in
[Building Peiosutils](docs/development/building.md), then run:

```sh
export PEIOS_SDK=/absolute/path/to/development-sdk
./bazel build //:peiosutils //:packages
./bazel test //...
```

`tools/check` also checks PSC formatting and runs Clippy through Bazel.
Development builds run on Linux x86_64. They do not qualify Peios kernel
operations or establish release readiness.

## Documentation and libraries

- [User and developer guides](docs/learn/) form part of Learn Peios.
- Installed applet manuals describe command options and exit behavior.
- [Library manuals](docs/man/) document `libreg`, `libregman`, and `libevman`.
- [Development guide](docs/development/building.md) covers SDK inputs,
  packages, generated targets, and validation.
- [Style adoption](docs/development/style.md) records compatibility exceptions.

The three public libraries ship shared objects and C headers. Their safe
Rust wrappers are `peios-reg`, `peios-regman`, and `peios-evman`. Their release
versions follow Peiosutils; the shared-library ABI major is 1.

## Origin and license

The fork preserves upstream history through
`873a7c75207e866ff9d67f16e0d204085f6412c6`. Upstream fixes are reviewed and
ported selectively; the project does not periodically merge upstream.
See [Upstream policy](UPSTREAM.md), [NOTICE](NOTICE), and [LICENSE](LICENSE).
