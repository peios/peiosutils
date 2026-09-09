# Upstream policy

peiosutils is a **hard fork** of [uutils/coreutils][uutils]. We do not
automatically track upstream and we do not contribute back. This document
records the fork point, the rules for selective syncs, and how upstream
security advisories are triaged.

[uutils]: https://github.com/uutils/coreutils

## Fork point

- **Upstream**: <https://github.com/uutils/coreutils>
- **Commit**: `873a7c75207e866ff9d67f16e0d204085f6412c6` ("README.md: update compatibility (#12302)")
- **Date**: 2026-05-15

The full upstream history is preserved on this branch. The upstream remote
is `uutils-upstream`, fetch only; push is disabled. It is per-clone state,
so a fresh clone has to add it:

```sh
git remote add uutils-upstream https://github.com/uutils/coreutils.git
git remote set-url --push uutils-upstream DO_NOT_PUSH
git fetch uutils-upstream main
```

## Why a hard fork

Two reasons:

1. **uutils' goal is GNU bug-for-bug compatibility.** That goal actively
   conflicts with peiosification of identity- and permission-aware
   commands (`ls -l`, `stat`, `chown`, `install`). The places where we
   most need to diverge are the places uutils most needs to converge.

2. **The commands likely to grow real security holes are the ones we are
   rewriting.** Pure text/data utilities (`cat`, `sort`, `wc`, etc.) are
   stable. The risky commands (FS traversal, perm handling, identity) are
   the ones we are replacing wholesale.

The second reason cuts both ways, and the first advisory pass (September
2026, 61 upstream advisories in under four months) showed how. Until a
risky command *has* been rewritten it carries upstream's bugs verbatim,
and a rewrite that keeps upstream's shape keeps upstream's races. The
security argument is therefore not "we don't need upstream's fixes"; it is
"we need to know about every one of them and decide each on its merits".
That is what the triage record below is for.

## What the `pu_` prefix means

Every applet crate in this tree is `pu_<name>`. The prefix means the crate
has been **reviewed for Peios** and either kept as-is (platform code
stripped) or rewritten; it does not mean the code no longer resembles
upstream. Many `pu_` crates are still the fork-point code minus non-Linux
paths. `git diff 873a7c752 HEAD -- src/uu/<name>` shows exactly how far a
crate has moved. The only `uu_` crates left are shared helpers
(`uu_base_common`, `uu_checksum_common`).

## Selective sync

We *may* take changes from `uutils-upstream` in three cases:

1. **A security fix in any applet we ship.** Whether the crate is
   kept-as-is or rewritten, read the advisory against *our* code and
   decide. If our code still has the flaw, port the fix: cherry-pick where
   the code is close to upstream, port by hand where it has diverged.
   Record the decision in `ADVISORIES.toml` either way.
2. **A substantial improvement in a kept-as-is applet** (perf,
   correctness, missing feature) that we have not rewritten. Cherry-pick
   the minimal patch.
3. **Tooling improvements** (build, test harness, locale plumbing) that
   apply to our fork unchanged.

We do **not** take:

- Non-security changes to a rewritten applet. Once rewritten, a command is
  on our trunk; upstream's direction there is by definition not ours.
- GNU-conformance test additions. We are diverging from GNU, not toward it.
- Multi-platform support (Android, BSDs, Windows, WSL). Peios is the
  only target.

## Advisory triage

`ADVISORIES.toml` at the repository root is the record. One entry per
advisory, from two sources:

- **Upstream advisories**: the GitHub security advisories published by
  uutils/coreutils
  (`https://api.github.com/repos/uutils/coreutils/security-advisories`).
- **Dependency advisories**: RustSec, matched against `Cargo.lock` (query
  OSV, ecosystem `crates.io`, or run `cargo deny check advisories`, whose
  configuration is in `deny.toml`).

Each entry carries the advisory id, the applet, severity, the upstream fix
commit when one exists, and a decision:

| status | meaning |
|---|---|
| `fixed` | our tree had the flaw and now carries a fix; `note` says what was ported and what tests ran |
| `not-affected` | our tree never had the flaw, or the applet is not shipped; `note` says why |
| `accepted` | the flaw is present and we are deliberately living with it; `note` says why and until when |
| `todo` | not yet decided |

An advisory that is not in the file is untriaged. Refreshing the list
against the sources and diffing it against the file is a mechanical step
(a pekit feature is proposed for it); the decisions are not.

## Releasing

A release is cut from `main` only when the `ci` workflow is green on the
commit being released. That is the whole rule, and it exists because the
opposite happened: the workflow was red from June to September 2026 (it
could not build the workspace at all — see the note in
`.github/workflows/ci.yml`), the red became the expected state, and 0.8.4
and 0.8.5, both security releases, were cut underneath it with no run of
the suite behind them beyond the nine-applet subset the packaging recipe
tests.

What "green" covers, so nobody mistakes it for less or more:

- `cargo check --workspace --locked`, with libpeios built from the
  revision `pekit.toml` pins.
- `cargo test --workspace --exclude uucore -- --skip gnu`: every unit and
  by-util test in the tree, on a plain Linux host. Applet behaviour that
  needs a Peios kernel (KACS ioctls, security descriptors) cannot run
  there and is exercised by `peios-integration-tests` against a booted
  image instead.
- The test step is not allowed to `continue-on-error`. A test that fails
  because upstream's expectation differs from the Peios model is rewritten
  to the Peios contract, or marked `#[ignore = "…"]` with the reason in
  the source, never skipped from the workflow.

To run the same thing locally, point the build at a libpeios checkout:

```sh
export PEIOS_LIB_DIR=…/libpeios/target/debug PEIOS_INCLUDE=…/libpeios/include
export PKM_UAPI=…/pkm/uapi LD_LIBRARY_PATH=$PEIOS_LIB_DIR
export BINDGEN_EXTRA_CLANG_ARGS="-isystem $(gcc -print-file-name=include)"
cargo test --workspace --exclude uucore --no-fail-fast -- --skip gnu
```

A release commit (`chore(release): X.Y.Z`) carries the version bump in
`Cargo.toml` and `Cargo.lock` and nothing else; fixes, test edits and
advisory bookkeeping go in their own commits before it, so the release
diff is reviewable at a glance and the tag points at exactly what CI ran.

## Procedure for a cherry-pick

```sh
git fetch uutils-upstream
git cherry-pick <sha>     # resolve conflicts as a Peios decision
git commit --amend        # rewrite the commit message in Peios style
                          # (conventional commits, no Co-Authored-By)
```

Always note the upstream SHA and rationale in the commit body, and for a
security fix, the advisory id.
