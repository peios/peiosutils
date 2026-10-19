---
title: chroot
type: reference
description: Run a command with a different directory as its root.
related:
  - peios/privileges/overview
  - peios/system-and-processes/overview
---

`chroot` runs a command with a **different directory as its root** — its `/`. Inside that command, the chosen directory *is* the top of the file system, and nothing above it can be named or reached.

```
chroot newroot [command [arg]...]
```

```
$ chroot /mnt/system /bin/sh
```

With a `command`, `chroot` changes the root to `newroot` and runs the command there. With no command, it starts an interactive shell.

## What it does

A process normally sees the whole file system, rooted at the real `/`. After `chroot`, the named directory becomes that process's `/`: a path like `/etc/config` inside the command resolves to `newroot/etc/config`, and there is no path that reaches outside `newroot` at all.

It is used to work inside another installed system — repairing or configuring a system image by entering it — and to run a command confined to a known subtree.

For the command to be usable, `newroot` must already contain what the command needs: the command's own executable, and whatever libraries and files it depends on, all present *under* `newroot`.

## Options

| Option | Effect |
|---|---|
| `--skip-chdir` | Do not change the working directory to `/` after changing the root. Permitted only when `newroot` is the current `/`. |

## A privileged operation

Changing a process's root directory is a **privileged** operation. `chroot` succeeds only for a caller whose token carries the right to do it, and is refused otherwise — the ability to redraw a process's view of the file system is not something every principal holds. See [Privileges](~peios/privileges/overview).

## Exit status

`chroot` exits with the command's own status once it finishes. Otherwise:

| Code | Meaning |
|---|---|
| `125` | `chroot` itself failed — for example, `newroot` is not a directory. |
| `126` | The command was found but could not be run. |
| `127` | The command was not found. |
