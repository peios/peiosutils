---
title: umount
type: reference
description: Detach a filesystem from the Peios mount tree by mount point or by source, with lazy, forced, recursive and all-targets variants.
related:
  - peios/mount-policies/overview
  - peios/mount-policies/mount
  - peios/mount-policies/lsblk
  - peios/mount-policies/managing-mounts
---

`umount` detaches a filesystem from the Peios mount tree. It is the counterpart of [`mount`](~peios/mount-policies/mount) and, like it, reads live mount state from the kernel (`/proc/self/mountinfo`) rather than any `/etc/mtab` — Peios has none. Each operand is resolved against that live state and unmounted with `umount2(2)`.

```
umount [-lfRA] [-dr] TARGET|SOURCE...
```

## Operands and how they are resolved

Each operand is either a **mount point** or a **source**:

- If the (canonicalised) operand is itself a mount point in the current mount table, it is unmounted directly. This is always unambiguous.
- Otherwise the operand is treated as a **source** and matched against the sources in the mount table. If it is mounted in exactly one place, that place is unmounted. If it is mounted in **several** places, `umount` refuses with an error naming the candidates — resolve it by naming a specific mount point, or use `-A` to unmount all of them.

If an operand matches nothing, it is "not mounted": normally an error (exit 32), but `-g` makes that a success and `-q` suppresses the message.

Several operands may be given in one invocation, and options may be interspersed with them (`umount /a -f /b`). Paths are handled as opaque bytes; an embedded NUL is a usage error. By default each operand is canonicalised; `-c` disables that and additionally selects `UMOUNT_NOFOLLOW` so the kernel does not follow a symlink in the final component.

## Recursive and all-targets unmounts

Two flags expand a single operand into multiple unmounts. Within one operand the expansion **stops on the first failure**.

- **`-R` / `--recursive`** unmounts the target and everything mounted underneath it, including any over-mount stack at a single point, ordered deepest-first.
- **`-A` / `--all-targets`** unmounts *every* mount point of the given source in the current mount namespace. This is the live-mountinfo "unmount everywhere" complement to source resolution; it is not an fstab feature.
- **`-A -R` together** compose: for each mount point of the source, recurse underneath it.

`-R` and `-r` are mutually exclusive (combining them is a usage error, exit 1).

## Flags

| Flag | Effect |
|---|---|
| `-l`, `--lazy` | Detach the filesystem now and clean up references later (`MNT_DETACH`). |
| `-f`, `--force` | Force the unmount (`MNT_FORCE`), e.g. for an unreachable server. |
| `-R`, `--recursive` | Unmount the target and everything under it (see above). |
| `-A`, `--all-targets` | Unmount every mount point of the given source (see above). |
| `-d`, `--detach-loop` | After unmounting, free the backing loop device. Best-effort and verified: it clears the device only if it still backs the mount just removed, so a recycled loop number is never clobbered. Usually redundant, since `mount`-created loops auto-clear on unmount. |
| `-r`, `--read-only` | If the unmount fails, remount the filesystem read-only instead (via `mount_setattr`). Mutually exclusive with `-R`. |
| `-g`, `--graceful` | Exit `0` when the target is absent or not mounted (rather than failing). Applied unconditionally. |
| `-q`, `--quiet` | Suppress "not mounted" messages. |
| `-c`, `--no-canonicalize` | Do not canonicalise paths; selects `UMOUNT_NOFOLLOW`. Applied unconditionally. |
| `-v`, `--verbose` | Say what is being unmounted. Repeatable. |
| `-N`, `--namespace NS` | Enter mount namespace `NS` (a PID, an ns file path, or a named namespace) before reading its mount table and unmounting. All work happens inside that namespace. |
| `-n`, `--no-mtab` | Accepted and ignored (no mtab on Peios). |
| `-i`, `--internal-only` | Do not invoke a `umount.<type>` helper (none ship). |
| `--fake` | Dry run: resolve operands and report, but skip the unmount syscalls. |
| `-h`, `--help` / `-V`, `--version` | Standard. |

The `umount2` flag mapping is direct: `-l` → `MNT_DETACH`, `-f` → `MNT_FORCE`, `-c` (or a symlinked final component) → `UMOUNT_NOFOLLOW`. `MNT_EXPIRE` is deliberately not exposed, matching util-linux.

### On privilege

As with `mount`, there is no coarse `uid==0` check: the applet is not set-user-ID and every unmount is authorised per-operation by KACS. Where util-linux silently suppresses an option for non-root users (`-c`, and `-g`'s effectiveness), Peios applies it unconditionally.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success (or a `-g` no-op on an absent target). |
| `1` | Incorrect invocation or a permission denial — including `-R` with `-r`, a missing operand, an ambiguous source, an embedded NUL, or an authorisation failure. |
| `2` | System error (e.g. cannot read the mount table). |
| `8` | Interrupted (SIGINT). |
| `32` | Unmount failed, or the target is not mounted (unless `-g`). |
| `64` | Several source/target arguments were given and at least one succeeded while another failed. |
| `126` | An external `umount.<type>` helper was found but failed to execute (moot until a helper ships). |

A single `-R` / `-A` / `-A -R` invocation stops on the first failure and yields `32`; the aggregate `64` only appears across multiple top-level arguments. To see what is currently mounted before unmounting, use [`mount`](~peios/mount-policies/mount) with no arguments or [`lsblk`](~peios/mount-policies/lsblk).
