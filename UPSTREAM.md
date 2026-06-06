# Upstream policy

peiosutils is a **hard fork** of [uutils/coreutils][uutils]. We do not
automatically track upstream and we do not contribute back. This document
records the fork point and the rules for selective syncs.

[uutils]: https://github.com/uutils/coreutils

## Fork point

- **Upstream**: <https://github.com/uutils/coreutils>
- **Commit**: `873a7c752` ("README.md: update compatibility (#12302)")
- **Date**: 2026-05-15

The full upstream history is preserved on this branch. The upstream remote
is configured locally as `uutils-upstream` for fetch only; push is disabled.

```sh
git remote -v
# uutils-upstream   https://github.com/uutils/coreutils.git (fetch)
# uutils-upstream   DO_NOT_PUSH (push)
```

## Why a hard fork

Two reasons:

1. **uutils' goal is GNU bug-for-bug compatibility.** That goal actively
   conflicts with peiosification of identity- and permission-aware
   commands (`ls -l`, `stat`, `chown`, `install`). The places where we
   most need to diverge are the places uutils most needs to converge.

2. **The commands likely to grow real security holes are the ones we are
   rewriting.** Pure text/data utilities (`cat`, `sort`, `wc`, etc.) are
   stable and unlikely to need security patches in practice. The risky
   commands (FS traversal, perm handling, identity) are the ones we are
   replacing wholesale — so the security-fix argument for tracking
   upstream is weaker than it first appears.

## Selective sync

We *may* cherry-pick from `uutils-upstream` in three cases:

1. **Critical security fix in a still-pristine `uu_*` command.** Cherry-pick
   the minimal patch, link upstream issue/CVE in the commit message.
2. **Substantial improvement in a still-pristine `uu_*` command** (perf,
   correctness, missing feature) that we have not yet peiosified. Same
   rule: cherry-pick the minimal patch.
3. **Tooling improvements** (build, test harness, locale plumbing) that
   apply to our fork unchanged.

We do **not** cherry-pick:

- Anything touching a `pu_*` command. Once peiosified, a command is on
  our trunk; upstream changes there are by definition wrong for us.
- GNU-conformance test additions. We are diverging from GNU, not toward it.
- Multi-platform support (Android, BSDs, Windows, WSL). Peios is the
  only target.

## Procedure for a cherry-pick

```sh
git fetch uutils-upstream
git cherry-pick <sha>     # resolve conflicts as a Peios decision
git commit --amend        # rewrite the commit message in Peios style
                          # (conventional commits, no Co-Authored-By)
```

Always note the upstream SHA and rationale in the commit body.
