---
title: mount
type: reference
description: Attach a filesystem to the Peios mount tree — new mounts, binds, moves, remounts and propagation changes — and apply a KACS mount policy at mount time.
related:
  - peios/mount-policies/overview
  - peios/mount-policies/policy-classes
  - peios/mount-policies/managing-mounts
  - peios/mount-policies/sd-storage-by-filesystem
  - peios/mount-policies/umount
  - peios/mount-policies/lsblk
  - peios/disks-and-filesystems/stratafs/overview
---

`mount` attaches a filesystem to the Peios mount tree. With no operands it instead *lists* what is currently mounted. It is a faithful reworking of the util-linux `mount(8)` surface for Peios, with two structural differences: Peios has **no `/etc/fstab` and no `/etc/mtab`**, so every mount is described entirely on the command line and live mount state is read from the kernel; and a `mount` can apply a **KACS mount policy** to the new filesystem at attach time (see [Mount policies](~peios/mount-policies/overview)).

```
mount [-t TYPE] [-o OPTIONS] SOURCE TARGET
mount [-o remount,OPTIONS] TARGET
mount --bind|--rbind|--move SOURCE TARGET
mount --make-shared|--make-private|... TARGET
mount [-l] [-t TYPE]
```

Internally `mount` uses the fd-based mount API (`fsopen` / `fsconfig` / `fsmount` / `move_mount`, plus `open_tree` and `mount_setattr`), never the classic single-shot `mount(2)`. This matters in one place you can see: when a mount fails, the kernel's `fs_context` message log is drained and printed as the reason, even without `-v`.

## Operands and argument shapes

Because there is no fstab to consult, operand resolution is strict:

- **No operands, no verb** — list mode (see [below](#list-mode)).
- **Two operands** — `SOURCE TARGET`.
- **One operand** — an error. In util-linux a lone operand is resolved through fstab; Peios has none, so a single operand cannot be turned into a `SOURCE TARGET` pair. Supply both, or use `--source` / `--target`.
- **A lone target** is valid only for `-o remount` and for a standalone propagation change (`--make-*`), which act on an existing mount point.

`--source SRC` and `--target DIR` name the operands explicitly and may be combined with a single positional. `--target-prefix DIR` prepends `DIR/` to the target after it is chosen. Options may be interspersed with operands (`mount SRC -o ro TGT`), matching util-linux.

Paths, `-o key=value` values, labels and UUIDs are handled as **opaque byte strings**, never assumed to be UTF-8; an embedded NUL is a usage error.

### Canonicalisation

By default source and target paths are canonicalised (made absolute, symlinks resolved). `-c` / `--no-canonicalize` disables that; `X-mount.nocanonicalize[=source|target]` is the `-o` form and can disable it for just one side. The final path handed to the kernel is protected against a TOCTOU symlink swap on its last component. This does not reject every symlink in the original input: default canonicalisation has already resolved it.

## Operation modes (verbs)

`mount` performs one of several operations. The **structural verbs** — bind, rbind, move, beneath and remount — are mutually exclusive with one another. A **propagation** change is *not* a structural verb: it may stand alone on a target, or trail another verb or a new mount in the same command (applied afterwards as one or more `mount_setattr` steps), and several may be combined.

| Verb | How to request it | What it does |
|---|---|---|
| **New mount** | `mount [-t T] SRC TGT` | Attach a fresh instance of the filesystem at `SRC` onto `TGT`. |
| **Bind** | `-B` / `--bind`, or `-o bind` | Make an existing subtree visible at a second location. |
| **Recursive bind** | `-R` / `--rbind`, or `-o rbind` | Bind a subtree together with every mount underneath it. |
| **Move** | `-M` / `--move`, or `-o move` | Relocate an existing mount to a new mount point. |
| **Move beneath** | `--beneath SRC TGT` | Attach `SRC` *beneath* the mount currently at `TGT` (the kernel enforces several constraints; violations surface as exit 32). |
| **Remount** | `-o remount[,...] TGT` | Change the options of an existing mount (see [Remounting](#remounting)). |
| **Propagation** | `--make-*` / `--make-r*`, or the `-o` tokens below | Change how mount/unmount events propagate across a subtree. |
| **List** | `mount` / `mount -l` | Print the current mounts. |

### Propagation flags

Each of these sets the propagation type of the mount at the target. The `--make-r*` (and `r`-prefixed `-o`) forms apply recursively to the whole subtree; the recursive form is all-or-nothing (it either applies to the entire subtree or to none of it).

| Flag | `-o` token | Recursive flag | Recursive `-o` token | Meaning |
|---|---|---|---|---|
| `--make-shared` | `shared` | `--make-rshared` | `rshared` | Events propagate to and from peer mounts. |
| `--make-slave` | `slave` | `--make-rslave` | `rslave` | Events propagate *in* from the master but not back out. |
| `--make-private` | `private` | `--make-rprivate` | `rprivate` | No propagation either way. |
| `--make-unbindable` | `unbindable` | `--make-runbindable` | `runbindable` | Private, and cannot be bind-mounted. |

A freshly created mount, bind or move is **private** by default unless its destination's parent is shared, in which case it joins that peer group — the standard kernel rule.

## Source specification

| Form | Resolution |
|---|---|
| Device path (`/dev/sda1`) | Used directly. |
| `-L LABEL` / `LABEL=`, `-U UUID` / `UUID=`, `PARTLABEL=`, `PARTUUID=` | Resolved to a device via libblkid. No match is a mount failure (exit 32). |
| A directory or regular file | Used directly (a bind source or target; a file may be a bind target). |
| A pseudo source (`tmpfs`, `proc`, `sysfs`, `none`, …) | Passed through; `-t` is required because it cannot be probed. |
| An image file | Attached through a loop device (see [Loop devices](#loop-devices)). |

## Filesystem type

`-t TYPE` names the type explicitly. `-t auto`, or omitting `-t` entirely, asks libblkid to **safely probe** the source; an ambiguous or multiply-signed device is refused (exit 32) rather than guessed at. `X-mount.auto-fstypes=LIST` constrains the probe candidates. Type *lists* and `no<type>` negation are not accepted in the mounting path (they remain meaningful only as a [listing filter](#list-mode)).

## The `-o` option language

`-o` takes a comma-separated list of `key` or `key=value` tokens and is repeatable (all occurrences are joined). A `key="..."` double-quoted value protects embedded commas; the `key=value` split is on the **first** `=`. Every token is sorted into one of the following categories.

### Per-mount attributes

Applied to the mount itself. Each accepts its negation; `ro`/`rw` additionally accept a `=vfs` / `=fs` / `=recursive` scope qualifier (bare is `=vfs`, non-recursive; `=fs` targets the superblock read-only flag; `=recursive` applies to the whole subtree).

| Option | Effect |
|---|---|
| `ro` / `rw` | Read-only / read-write. |
| `suid` / `nosuid` | Honour / ignore set-user-ID bits. |
| `dev` / `nodev` | Allow / disallow device nodes. |
| `exec` / `noexec` | Allow / disallow execution. |
| `atime` / `noatime` | Update / never update access times. |
| `relatime` / `norelatime` | Relative-atime updates on or off. |
| `strictatime` / `nostrictatime` | Strict-atime updates on or off. |
| `diratime` / `nodiratime` | Directory access-time updates on or off. |
| `nosymfollow` | Do not follow symlinks on this mount. There is no positive `symfollow` token — the attribute is cleared only via a remount mask. |

The atime tokens share a single mode field; the last one specified wins.

### Superblock flags

| Option | Effect |
|---|---|
| `sync` / `async` | Synchronous / asynchronous writes. |
| `dirsync` | Synchronous directory updates. |
| `lazytime` / `nolazytime` | Lazy on-disk timestamp updates on or off. |
| `iversion` / `noiversion` | Inode version counting on or off. |
| `silent` / `loud` | Suppress or emit certain kernel messages. |
| `mand` / `nomand` | Obsolete (removed from the kernel). Accepted and ignored with a note under `-v`. |

### Filesystem-specific parameters

Any token not recognised above is forwarded verbatim to the filesystem: `key=value` as a string parameter, a bare `key` as a flag. There is no built-in allow-list and no length limit; a rejection by the filesystem is reported with the offending key and the kernel's message.

For example, StrataFS takes its precedence-ordered directory stack through
`strata=`:

```sh
mount -t stratafs none /bin -o 'strata=/lcl/bin+create:/usr/bin+ro'
```

See [StrataFS](~peios/disks-and-filesystems/stratafs/overview) for the stack flags, merge
and write-routing model, and the `stratafs` inspection command.

### Userspace-only tokens

These never reach the filesystem. They include the meta-verbs `remount`, `bind`, `rbind`, `move`; the loop controls `loop` / `loop=/dev/loopN`, `offset=`, `sizelimit=` (numeric values accept `K`/`M`/`G`/`T` and `KiB`/`MiB`/… suffixes); the propagation tokens listed [above](#propagation-flags); and `defaults`, which expands to `rw,suid,dev,exec,async` (later tokens override it).

### Functional `X-mount.*` options

| Option | Effect |
|---|---|
| `X-mount.mkdir[=mode]` | Attempt to create missing target directories, like `-m`. In the [reviewed implementation](#source-verified-execution-limits), the requested mode is not applied. |
| `X-mount.subdir=DIR` | Attach subdirectory `DIR` of a freshly mounted filesystem at the target. Effective only for a new-instance mount; silently ignored (noted under `-v`) for bind/move/remount/propagation. |
| `X-mount.noloop` | Suppress the implicit loop device for a regular-file source. |
| `X-mount.auto-fstypes=LIST` | Constrain the `-t auto` probe to these types. |
| `X-mount.nocanonicalize[=source\|target]` | The `-o` form of `-c`; with `=source` or `=target` it disables canonicalisation for just that path. |

`X-mount.idmap` and `X-mount.owner` / `group` / `mode` are **not supported** and are rejected with a usage error: Peios ownership is SID/security-descriptor based, so the fix is to add a SID/ACE to the descriptor rather than remap or chown.

### KACS mount policy

This is the genuinely Peios-specific part of `mount`. A mount policy is a per-superblock setting that governs how FACS treats the filesystem; the full model is described in [Mount policies](~peios/mount-policies/overview) and its detail pages. `mount` can set that policy at attach time:

| Option | Policy class |
|---|---|
| `-o policy=deny-missing` | `facs_deny_missing` — a file with no SD is unreachable. |
| `-o policy=synth-ephemeral` | `facs_synthesize_ephemeral` — a missing SD is synthesised in memory only. |
| `-o policy=synth-persist` | `facs_synthesize_persistent` — a missing SD is synthesised and written back. |
| `--synth-sddl SDDL` | Provide the mount-level SD template used during synthesis (only valid with a `synth-*` policy). |

`policy=unmanaged` is **not** user-settable; only the kernel sets the unmanaged class, for its own pseudo-filesystems. `policy=` is valid only on a **new mount** of a real filesystem — combining it with bind/move/remount/propagation, or with list mode, is a usage error. See [Policy classes](~peios/mount-policies/policy-classes) for what each class does and [SD storage by filesystem](~peios/mount-policies/sd-storage-by-filesystem) for how the SD is physically stored.

The policy is applied to the *detached* filesystem before it is attached: if setting it fails, that filesystem is not attached with an unintended policy. This is an attachment guarantee, not whole-command rollback: target creation may already have occurred. In kernel source `8e0e22de3a59cad506bbbf8873de456e16ad272d`, the policy gate accepts enabled `SeManageVolumePrivilege` or `SeTcbPrivilege` and requires successful privilege-use marking. Failure of that gate gives `EPERM` (exit 1) and no attachment. `--synth-sddl` is validated client-side first — it must be well-formed SDDL and must include an owner.

> [!WARNING]
> The [peiosutils 0.8.18 diagnostic](https://github.com/peios/peiosutils/blob/3344d4690476fd66bfaec99b1ae92190bbcba06f/src/uu/mount/src/policy.rs#L84-L99) still says the policy operation requires `SeTcbPrivilege`. That is not the full gate in the pinned kernel. See [Check policy access](~peios/mount-policies/managing-mounts#check-policy-access) before interpreting this as a need to grant TCB; administrator membership alone is not enough, and automatic escalation or a global `CAP_SYS_ADMIN` remap is not a remedy.

Peios applies no coarse `uid==0` check anywhere: the `mount` applet is not installed set-user-ID, and every privileged action is authorised per-operation by KACS.

### Remounting

`-o remount` changes the options of an existing mount without detaching it. Peios does **not** perform the util-linux "read the old flags and re-supply them" dance — the fd-based API applies deltas rather than resetting, so each remount touches only what you name. Per-mount attributes (`ro`/`rw`, `nosuid`, atime, …) are changed via `mount_setattr`; superblock flags, filesystem parameters and `ro=fs`/`rw=fs` go through the superblock reconfigure path. `=recursive` remounts the whole subtree, all-or-nothing.

A **bind** mount shares its source's superblock, so any superblock-level option on a bind remount (a Category-B flag, a filesystem parameter, or `ro=fs`/`rw=fs`) is refused as a usage error rather than silently mutating the shared superblock for every mount of that filesystem. Only per-mount VFS attributes are valid on a bind remount.

## Loop devices

A regular-file source is backed by a loop device automatically **when the type is unspecified or the filesystem is recognised by libblkid**; `X-mount.noloop` suppresses this. `-o loop` forces auto-allocation of a free device, `loop=/dev/loopN` names one, and `offset=` / `sizelimit=` select a region of the file (they imply loop and are an error against a real block device). A backing file already attached at the same offset and size is *reused* rather than doubly attached, to avoid corruption. Loops created by `mount` are auto-cleared by the kernel on unmount, so they do not leak; `umount -d` force-clears the rest (see [`umount`](~peios/mount-policies/umount)).

## Other flags

| Flag | Effect |
|---|---|
| `-t`, `--types TYPE` | Filesystem type, or `auto`. |
| `-o`, `--options LIST` | Mount options (above); repeatable. |
| `-r`, `--ro`, `--read-only` | Mount read-only (`-o ro`). |
| `-w`, `--rw`, `--read-write` | Mount read-write and **forbid** the automatic read-only fallback on a write-protected device (`-o rw`). |
| `--source SRC` / `--target DIR` | Name the operands explicitly. |
| `--target-prefix DIR` | Prepend `DIR/` to the target. |
| `-B`, `--bind` / `-R`, `--rbind` / `-M`, `--move` / `--beneath` | The structural verbs. |
| `--make-*`, `--make-r*` | Propagation changes. |
| `--exclusive` | Force a unique superblock instance (no reuse). Meaningful only for multi-instance filesystems (e.g. `tmpfs`) or read-only block mounts; on an already-mounted writable block device it fails with `EBUSY` (exit 32). |
| `-m`, `--mkdir[=MODE]` | Attempt to create missing target directories. The requested MODE is parsed but not applied at the [reviewed source revision](#source-verified-execution-limits). |
| `-L`, `--label LABEL` / `-U`, `--uuid UUID` | Select the source by filesystem label / UUID. |
| `-c`, `--no-canonicalize` | Do not canonicalise paths. |
| `-f`, `--fake` | Skip mount and policy syscalls; target preparation still runs. With `--mkdir` or `X-mount.mkdir`, directories may be created. See [execution limits](#source-verified-execution-limits). |
| `-v`, `--verbose` | Narrate resolved values and each syscall; drain the kernel `fs_context` log. Repeatable but `-vv` is the same as `-v`. |
| `-l`, `--show-labels` | In list mode, append each filesystem's label. |
| `--onlyonce` | Accepted and stored, but unused by the [reviewed execution path](#source-verified-execution-limits). Do not rely on it to skip an existing mount or make repeat startup safe. |
| `-N`, `--namespace NS` | Operate inside mount namespace `NS` (a PID, an ns file path, or a named namespace). Source resolution happens in the caller's namespace; the mount lands in the target namespace. |
| `-i`, `--internal-only` | Do not invoke a `mount.<type>` helper. |
| `-n`, `--no-mtab` | Accepted and ignored (Peios has no mtab). |
| `--synth-sddl SDDL` | KACS synth-policy template SD (above). |
| `-h`, `--help` / `-V`, `--version` | Standard. |

No external mount helpers ship in this version, so a network or FUSE type fails with "no helper for type" (exit 1) — a clean deferral, not a crash.

## Source-verified execution limits

These limits were checked in peiosutils `3344d4690476fd66bfaec99b1ae92190bbcba06f`.
They describe that implementation, not a runtime test or a historical release
boundary; check the version in the image you operate.

- **No already-mounted guard from `--onlyonce`.** The [request parser](https://github.com/peios/peiosutils/blob/3344d4690476fd66bfaec99b1ae92190bbcba06f/src/uu/mount/src/request.rs#L148-L168)
  stores it, but the [executor](https://github.com/peios/peiosutils/blob/3344d4690476fd66bfaec99b1ae92190bbcba06f/src/uu/mount/src/flow.rs)
  never consults it. Inspect existing mounts before a repeat operation; this
  finding does not mean every repeat necessarily succeeds or creates an overmount.
- **`--fake` can create targets.** [Target preparation](https://github.com/peios/peiosutils/blob/3344d4690476fd66bfaec99b1ae92190bbcba06f/src/uu/mount/src/flow.rs#L37-L65)
  precedes the [new-mount fake return](https://github.com/peios/peiosutils/blob/3344d4690476fd66bfaec99b1ae92190bbcba06f/src/uu/mount/src/flow.rs#L117-L120).
  With either mkdir option it may write directories. It is not a zero-write
  preview or proof that a real mount's authority and filesystem checks will pass.
- **MODE is not target security.** The [creation helper](https://github.com/peios/peiosutils/blob/3344d4690476fd66bfaec99b1ae92190bbcba06f/src/uu/mount/src/flow.rs#L483-L502)
  discards the requested mode and ignores the directory-creation result before
  continuing path preparation. Do not rely on `--mkdir=0700` for privacy.
  Establish and verify the target's [native security](~peios/file-permissions/managing-file-security)
  separately; a parsed option is not a verified descriptor.
- **Failure can follow attachment.** [New-mount execution](https://github.com/peios/peiosutils/blob/3344d4690476fd66bfaec99b1ae92190bbcba06f/src/uu/mount/src/flow.rs#L126-L176)
  sets policy before attachment, but a subsequent propagation step can fail
  after the mount is attached. Retain diagnostics and inspect the live source,
  target, filesystem, writable state and policy before retrying. Do not infer
  an unchanged system from a nonzero exit status.

These gaps matter for [startup integration](~peios/mount-policies/managing-mounts#mount-non-root-storage-at-startup):
`--onlyonce` is not an idempotence mechanism, and a failed or stopped command
is not by itself a verified detach or recovery procedure.

## List mode

With no operands, `mount` prints one line per mount, read from `/proc/self/mountinfo`:

```
SOURCE on TARGET type FSTYPE (OPTIONS)
```

`mount -t TYPE` with no operands *filters* the list by filesystem type instead of mounting; here type lists (`-t ext4,xfs`) and `no<type>` negation (`-t nosysfs`) are honoured. `-l` appends ` [LABEL]` to each line when a label is known.

Details of the rendering: the option field is the positional VFS-then-superblock merge with a coalesced `ro`/`rw`, not sorted; mountinfo octal escapes are decoded; control characters in the mount point become `?`; and the source is shown as the kernel recorded it, with `/dev/loopN` resolved to its backing file and `/dev/dm-N` to `/dev/mapper/<name>`. The listing shows only mounts the caller's token may observe; a partial or empty view is correct, not an error, and the paths of filtered-out parents are never reconstructed.

For a device-oriented view of what is available to mount, use [`lsblk`](~peios/mount-policies/lsblk).

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | Incorrect invocation or a permission denial — bad flags, mutually-exclusive verbs, a lone operand, an invalid `policy=` or `--synth-sddl`, an authorisation failure (`EPERM`/`EACCES`), an embedded NUL, or "no helper for type". |
| `2` | System error (out of memory, no free loop device, cannot fork). |
| `4` | Internal error (an invariant failure). |
| `8` | Interrupted (SIGINT), after signal-safe cleanup. |
| `32` | Mount failure — a syscall in the flow failed, a label/UUID had no match, a probe was undetermined, or a `--beneath`/move/`--exclusive` kernel refusal. |
| `126` | An external `mount.<type>` helper was found but failed to execute (moot until a helper ships). |

Code `16` (mtab) is never produced. Code `64` (some succeeded, some failed) only arises when several source arguments are given and at least one succeeds while another fails.
