---
title: Mount Operations and Their Boundaries
type: concept
description: >-
  Understand attachment, policy, and partial failure before changing the
  mount tree.
---

`mount` attaches a filesystem or changes an existing mount. With no operands,
it reads live mount state rather than attaching anything.

## Source, target, and policy are separate choices

A source identifies the storage or filesystem being mounted. A target is where
its contents become visible. A mount policy controls how the Peios access
model treats objects on that filesystem.

A new filesystem's policy is set before attachment. Failure to set that policy
prevents attachment with an unintended policy. This does not make the whole
command transactional: target creation or later propagation changes have their
own effects.

Bind mounts share a source filesystem rather than copying it. Superblock
options can affect shared state, while per-mount attributes concern the
selected view. The command rejects superblock changes in a bind remount
instead of silently changing every view of the filesystem.

## Inspect live state before and after a change

This implementation does not use `/etc/fstab` to supply missing operands or
`/etc/mtab` as its source of truth. It reads the kernel's mount table. Listing
and attachment are different modes, so a listing filter is not necessarily a
valid attachment type expression.

A filesystem probe can refuse an ambiguous device rather than choosing between
conflicting signatures. Inspect the source and resolve ambiguity before
attempting an attachment.

## Failure is not a rollback report

A propagation change can fail after a filesystem has been attached. A nonzero
status therefore does not establish that the target stayed unchanged. Retain
diagnostics and inspect the live mount before retrying.

The accepted `--onlyonce` flag is not enforced by the executor. It does not
make repeated invocation idempotent. Check existing attachments explicitly.

Fake mode skips mount syscalls, but target preparation can still create
directories. It does not prove that a real operation will pass authorization
or filesystem checks. A requested mkdir mode is not a verified Peios security
descriptor; establish the target's security separately.

## Keep storage lifecycle visible

Regular-file images can need loop devices. The command can allocate or reuse
suitable loop backing, and newly allocated loops use automatic cleanup. That
mechanism does not make an image safe to modify through uncoordinated
concurrent access.

`mount(1)` defines source syntax, policy restrictions, loop handling,
namespace selection, and status codes. The filesystem's own documentation owns
its specific parameters.
