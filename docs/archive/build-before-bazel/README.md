# Build tooling before Bazel

These files are retained as migration history. They are not supported build
entry points. The former recipe, package definitions, and SDK helper refer to
retired Pekit and
PKM inputs. The old utility-list and Clippy helpers assume the upstream
workspace names.

Use `bazel`, `tools/check`, and the maintained development SDK preparation
helper described in `docs/development/building.md` at the repository root.

The inherited traversal probe assumes POSIX ownership commands and infers
security from syscall-name counts. It is preserved as historical evidence,
not a Peios race-safety qualification. The old removal benchmark also targets
the upstream Cargo binary layout.
