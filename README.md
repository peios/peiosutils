# peiosutils

Peios core userspace utilities. A hard fork of
[uutils/coreutils](https://github.com/uutils/coreutils), being peiosified
one command at a time.

## What this is

Peios doesn't use the GNU coreutils. It doesn't use POSIX permissions,
doesn't expose UIDs/GIDs as security primitives, and uses KACS
(Kernel Access Control Subsystem) for authorisation. Most of what
coreutils does — text processing, file I/O, hashing — is the same on
any kernel. A handful of commands (`ls -l`, `stat`, `chown`, `install`)
need real rewrites against KACS instead of POSIX. A few more (`chmod`,
`chgrp`, `chcon`, `newgrp`) don't translate at all and get deleted.

uutils gave us a clean, MIT-licensed, modular Rust starting point with
all the boring stuff already working. We're forking it (hard fork — no
auto-sync from upstream) and walking through every command alphabetically,
peiosifying as we go.

## Peiosification status

Each command lives in its own crate under `src/uu/<name>/`.

- `uu_<name>` — pristine from uutils, not yet reviewed.
- `pu_<name>` — peiosified (reviewed, kept-or-rewritten, KACS-aware
  where it needs to be).

At any time, `ls src/uu/` is a rough progress bar for the peiosification
pass. Commands that don't translate to Peios get deleted entirely; their
absence is the same signal as a removed crate.

See [`UPSTREAM.md`](UPSTREAM.md) for upstream-sync policy.
See [`NOTICE`](NOTICE) for attribution.

## Building

```sh
cargo build --release            # multiplexed peiosutils binary
cargo build --release -p uu_cat  # individual command (still uu_* prefixed)
```

The multiplexed binary supports busybox-style symlink dispatch: a symlink
named `cat → peiosutils` invokes the `cat` command. Individual builds are
also supported.

## Origin

Forked from `uutils/coreutils` at SHA `873a7c752` (May 2026). The original
remote is preserved as `uutils-upstream` (fetch-only). All git history
prior to the fork is the original uutils history, retained for
attribution and for selective cherry-picks of upstream security fixes.

## License

MIT, same as uutils. See [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE).
