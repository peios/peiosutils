# Building Peiosutils

Use pekit to build Peiosutils and run its tests. Pekit builds each crate on
its own, so a change rebuilds only the crates it reaches. The supported
development host is x86-64 Linux.

## Before you start

You need pekit and Git. Everything else is pinned and fetched by pekit on
the first build: the Bootlin C toolchain, Rust 1.98.1 with Clippy, busybox,
libclang, and the crates `Cargo.lock` selects. The first build needs
network access; later builds find what they need in pekit's store, and
every step runs without network access in a sandbox that holds only its
declared inputs.

The build files use the gcslib libraries named in `workspace.gcsl` and the
build files, and the libraries of peios-rs, the Peios SDK, and authd.
`pekit lock` pins their commits in `pekit.lock`. Pekit builds libpeios and
its headers from the Peios SDK's own files, and libauth from authd's.

## Build

```sh
pekit build peiosutils uudoc libstdbuf libreg libregman libevman include
```

| Target | Output |
|---|---|
| `peiosutils` | The multicall executable, with every utility of a Unix host |
| `uudoc` | The tool that writes the utilities' manuals and shell completions |
| `libstdbuf` | The library `stdbuf` preloads into the program it runs |
| `libreg` | The registry's C library, `libreg.so.1` by its shared object name (SONAME) |
| `libregman` | The registry manual's C library, `libregman.so.1` by its SONAME |
| `libevman` | The event manual's C library, `libevman.so.1` by its SONAME |
| `include` | The C libraries' headers |

Pekit prints where each output is, under `pekit-out/<target>/`. The
multicall executable and `uudoc` are built with the features
`feat_os_unix`, `feat_external_libstdbuf`, and `uudoc`. They load
`libpeios.so.0` and `libauth.so.1`, and libreg loads `libpeios.so.0`, by
their SONAMEs; none carries a runtime search path.

With `feat_external_libstdbuf`, `stdbuf` preloads the installed libstdbuf,
from beside its executable or from `/usr/lib/x86_64-linux-peios/peiosutils`,
the directory its build is told. Its build script embeds no library then.

The executables carry each utility's English messages, which uucore's
build script reads from each utility's `locales` directory. Other languages
are read at run time from `locales/<utility>/` beside the executable.

## Test

```sh
pekit test
```

| Test | Check |
|---|---|
| `units` | Each member's unit tests, but the multicall executable's and the C library wrappers' |
| `wrappers` | The unit tests of `peios-reg`, `peios-regman`, and `peios-evman`, linked with the C libraries pekit builds |
| `peiosutils` | The multicall executable's own tests and the integration tests, run against it as an installation lays it out |
| `abi` | A C11 program calls each C library through its header, and a C++17 file includes the headers, with every warning an error |
| `doctests` | Each member's documentation tests |
| `clippy` | Clippy's checks of every target of every member, tests among them, with every warning denied and the workspace's `.clippy.toml`, but the `cargo` group, which reads the workspace's manifests through Cargo |

The integration tests run the multicall executable, with libstdbuf and the
translations beside it, and each utility's fixtures. They skip the GNU
conformance tests, as the maintained suite always has, and the tests in
`HOST_ONLY` in `tests.gcsl`. Those need what only a host or a booted system
has: GNU tools, extended attributes, a user database and login, device
nodes, a resolvable host name, or files the user does not own. Each is
listed with its reason; qualify them on a disposable guest. The suite runs
four tests at a time and is retried up to twice, as the CI profile in
`.config/nextest.toml` allows: the `tail --follow` tests time what they
watch and can miss it on a loaded machine. A failure in the code fails
every run. Tests that run shell
commands are given busybox's `sh` and tools. A test whose inputs did not
change is found in the store and not run again. The tests do not qualify
Peios kernel operations.

For the full contributor check, install the pinned PSC tools into a
directory outside the repository, then run the shared entry point:

```sh
python3 tools/prepare-style-tools.py "$HOME/.cache/peiosutils-style"
export PEIOS_STYLE_BIN="$HOME/.cache/peiosutils-style/bin"
tools/check
```

`tools/check` checks owned source formatting and lint with rustfmt, Ruff,
ShellCheck, shfmt, clang-format, and Prettier, then runs `pekit test`.
`tools/style-check.py --fix` applies mechanical formatting. Fixture inputs,
archived records, dependency notices, and vendored code retain their
original format. GitHub's workflow runs the style checks alone; builds and
tests run with pekit.

Run `TRAIL=/absolute/path/to/trail tools/check-docs` for the component tree.
The checker compares external-link diagnostics with a reviewed baseline.
Use `--context ../learn` for the merged publication check; this separate
check resolves links owned by other components.

## How the build is planned

`build.gcsl` plans the Cargo workspace with the gcslib rust library's
`workspace`: Cargo works out each member's build, unit by unit, and pekit
compiles each unit as a step. Planning reads the manifests, `Cargo.lock`,
and which files exist, never the sources, so editing a source plans nothing
again. Each member's crates are compiled with the lints in the workspace's
`[lints]` table, as Cargo compiles them. uucore's compiles are given each
utility's translations, not the utilities' sources, so editing a utility
does not rebuild uucore.

Keep Rust dependency versions in the Cargo manifests and `Cargo.lock`, and
update the lockfile deliberately with Cargo. `peios-sys` generates its
bindings from the SDK's headers with bindgen and links libpeios, and
`peios-auth` links authd's libauth, so each is given what its library
provides. The commits `pekit.lock` pins say which SDK, which peios-rs, and
which authd; the rust library fails a build whose `Cargo.lock` takes their
crates at another commit than `pekit.lock` pins their libraries at.

The C library wrappers, `peios-reg`, `peios-regman`, and `peios-evman`,
link the C libraries pekit builds, by the directories their build scripts
are told. libreg needs libpeios, which `peios-reg`'s build script tells the
linker to find beside libreg.

Generated manuals use `SOURCE_DATE_EPOCH` when it is set. Direct `uudoc
manpage` invocation uses today's date unless `SOURCE_DATE_EPOCH` supplies a
non-negative whole number of seconds since the Unix epoch; the supplied
timestamp is formatted in UTC. Invalid values fail before writing a manual.

## Building against peiosutils

Peiosutils is a GCSL library as well as a workspace. A workspace whose
crates depend on `peios-reg`, `peios-regman`, or `peios-evman` gives each
the C library it links, and gives its programs those libraries, and the
libpeios libreg loads, when they run:

```text
import "github.com/peios/peios-rs" as peios_rs
import "github.com/peios/peiosutils"

rust.workspace(..., crates = [...peios_rs.crate_settings(), ...peiosutils.crate_settings()])
rust.run_test(test, given = [...peios_rs.runtime(), ...peiosutils.runtime()], env = peiosutils.runtime_env())
```

`pekit lock` pins peiosutils's commit in the workspace's `pekit.lock`, and
authd's, whose libauth peiosutils's build imports. The rust library fails
a build whose `Cargo.lock` takes peiosutils's crates at another commit than
that, so the wrappers and the libraries they link always agree.

## Not built with pekit yet

Packages, debug information, and builds with the native Peios toolchain,
which hardens programs with control-flow enforcement technology (CET), are
not yet built with pekit. The package definitions, their checks, and the
native build settings from the Bazel build are kept outside this repository
until they are. Passing host development checks does not qualify the
native configuration or a release image.
