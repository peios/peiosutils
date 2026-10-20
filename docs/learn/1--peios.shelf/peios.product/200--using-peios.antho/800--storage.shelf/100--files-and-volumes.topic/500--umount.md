---
title: Detaching a Mounted Filesystem
type: concept
description: >-
  Understand target selection, busy mounts, and deferred cleanup.
related:
  - peios/files-and-volumes/mount
  - peios/files-and-volumes/lsblk
---

`umount` detaches a filesystem from the mount tree. Its operand can name a
mount point or a source resolved through live mount state.

## Name the intended attachment

A source mounted at several locations is ambiguous. Name the particular mount
point when only that attachment should be removed. All-targets mode is an
explicit request to expand the operation to every matching mount.

Recursive mode expands beneath a target and works from deeper mounts upward.
Expansion can stop on a failure, leaving a partially changed tree. Inspect
what remains before retrying.

## Detachment and final cleanup can differ

An ordinary unmount can fail while resources are busy. Lazy unmount removes
the attachment from the visible tree while deferring cleanup until references
are released. It is not a guarantee that every user has stopped using the
filesystem or that a device is ready to remove.

Forced unmount is another distinct request whose meaning depends on the
filesystem. Read-only fallback changes state instead of proving a detach. Use
the mode that matches the recovery task, rather than escalating flags until a
command returns success.

## Check the resulting state

The command reads `/proc/self/mountinfo`, including after entering a selected
mount namespace. A query in another namespace can show a different set of
attachments.

An aggregate failure can coexist with successful top-level operands.
`umount(1)` defines ambiguous-source handling, expansion stopping rules, loop
cleanup, and the status distinctions.
