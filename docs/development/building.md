# Building Peiosutils

Bazel is the build and packaging entry point. It compiles workspace Rust crates
as native targets. Cargo resolves the dependency lockfile and remains available
for development; Bazel does not run Cargo to compile the workspace.

The development configuration targets Linux x86_64 with Rust 1.98.1 and Bazel
8.4.2. The launcher verifies its Bazel download. Install a C/C++ compiler,
Python 3.11 or later, binutils, libclang, pkg-config, and the development headers
required by the locked native dependencies.

## Prepare the development SDK

Use the consolidated `peios-sdk` and maintained `authd` repositories.
The Rust bindings are pinned in
`Cargo.lock`; do not substitute the retired PKM workspace.

```sh
python3 tools/prepare-dev-sdk.py ../peios-sdk /absolute/path/to/development-sdk \
    --auth-source ../authd
export PEIOS_SDK=/absolute/path/to/development-sdk
```

The destination must be new and outside the source checkout. The helper builds
both SDKs with their own Bazel targets, installs matching headers and libraries,
and records the source commit, local changes, kernel snapshot, and file hashes.
It includes `libpeios.so.0` and `libauth.so.1`. Use authd revision
`a014fa4c74f17ce4f67ba964eefce33a1a3dbdfa` to match the Rust client pin.
This is a host development SDK, not evidence of native OS qualification.

## Build binaries and packages

```sh
./bazel build //:peiosutils //:uudoc
./bazel build //:packages
```

The multicall executable is `bazel-bin/peiosutils`. For direct execution on a
host without an installed Peios SDK, add its library directory to
`LD_LIBRARY_PATH`. Bazel test launchers supply their declared library paths.

`bazel-bin/packages.store` contains content-addressed blobs, canonical PPK
manifests, and an index with build and package metadata. The family contains:

- `peiosutils`: the multicall executable, applet aliases, and stdbuf preload library.
- `peiosutils-common`: manuals, shell completions, and translations.
- `libreg`, `libregman`, and `libevman`: public shared libraries.
- The corresponding `-devel` packages: headers, linker names, and pkg-config files.
- `peiosutils-debuginfo`, `peiosutils-debugsource`, and `peiosutils-source`.

Runtime packages require their declared shared-library dependencies. The three
public libraries use ABI major 1; their release version follows Peiosutils.
The source package includes the build graph and tools needed to regenerate it.

Development packages are unsigned and explicitly marked as development output.
The package checks verify ELF properties, debug splits, library names, command
artifacts, dependency notices, package boundaries, and blob identities. They do
not establish that privileged operations work on a Peios kernel.

Generated manuals use `SOURCE_DATE_EPOCH=0` during package assembly and gzip
headers use the same fixed epoch. This is a reproducibility timestamp, not a
release date. Direct `uudoc manpage` invocation uses today's date unless
`SOURCE_DATE_EPOCH` supplies a non-negative whole number of seconds since the
Unix epoch; the supplied timestamp is formatted in UTC. Invalid values fail
before writing a manual.

## Update dependencies and generated targets

After changing a Cargo manifest, update and review `Cargo.lock`. Then resolve
and regenerate the native graph using the pinned Cargo version:

```sh
python3 tools/bazel-graph.py --fetch
python3 tools/bazel-targets.py --buildifier /absolute/path/to/buildifier
python3 tools/bazel-graph.py --check
./bazel test //:generated-targets-test
```

Use the hash-pinned buildifier from `tools/prepare-style-tools.py`. Without
`--fetch`, the resolver uses cached sources only. It reads locked metadata and
production dependency edges so benchmark-only optional dependencies do not
enter production binaries.

Review changes to `build/crate-graph.json` alongside the manifests. Put
handwritten root targets in `build/root.BUILD.bazel`; generated workspace
`BUILD.bazel` files are checked against the graph. The stdbuf preload library
is a separate build and package target, not a link dependency of the multicall
executable.

## Run focused checks

```sh
./bazel test //crates/registry-manual:unit-tests //src/uu/mkuki:unit-tests
./bazel test //tests/abi:c-tests
./bazel test //packages:assembly-test //packages:store-test \
    //packages:licences-test //packages:family-test
```

The integration harness is `//:integration-tests`. An argument such as
`--test_arg=test_uudoc` selects a test group. Keep inherited compatibility tests,
intentional Peios differences, and native-only qualification distinct when
reviewing results.

## Native build configuration

`--config=peios` selects an explicitly supplied Peios Rust toolchain and SDK.
Use the isolated native build environment described by that toolchain's
qualification record. Native package checks additionally require CET and
packed relocations. Passing host development checks does not qualify this
configuration or a release image.

## Style and continuous integration

Install the pinned PSC tools outside the source checkout, then run the
maintained checks:

```sh
python3 tools/prepare-style-tools.py /absolute/path/to/style-tools
export PEIOS_STYLE_BIN=/absolute/path/to/style-tools/bin
tools/check
```

`tools/style-check.py --fix` applies mechanical formatting. Fixture inputs,
archived records, dependency notices, and vendored code retain their original
format. Rust source follows the workspace's pinned rustfmt configuration.

`tools/check` runs the declared Bazel test graph and the rules_rust Clippy
aspect. Cargo is used for formatting and dependency-graph maintenance, not
as an alternative compiler driver in CI. CI builds the SDK from recorded
commits and uses a pinned Trail compiler to validate Learn documents.

Run `TRAIL=/absolute/path/to/trail tools/check-docs` for the component tree.
The checker compares external-link diagnostics with a reviewed baseline.
Use `--context ../panorama/docs` for the merged publication check; this
separate check resolves links owned by other components.

Bazel inherits source lint levels and `.clippy.toml` from the Cargo workspace.
Cargo metadata lints are omitted inside Bazel compiler actions because those
actions do not invoke Cargo; locked dependency metadata is checked by the
graph resolver.
