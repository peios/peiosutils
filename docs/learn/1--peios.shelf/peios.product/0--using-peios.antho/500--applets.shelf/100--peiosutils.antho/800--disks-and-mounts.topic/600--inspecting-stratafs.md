---
title: Inspecting StrataFS
type: how-to
description: Trace merged paths, review local overrides, and interpret write routing without changing the stack.
---

Use `stratafs` to inspect the current mount namespace before changing a merged
path. It does not elevate the caller or modify files. An incomplete protected
view fails rather than pretending to be a complete answer.

```sh
stratafs list
stratafs resolve /bin/sh
stratafs origin /bin/sh
```

The stack lists strata in precedence order. A path can have one provider,
merged directory participants, shadowed same-type objects, masked different
types, or an absent entry. The origin report names the actual providers;
write and delete reports explain routing, not authorization.

A write can happen in place, copy up to the create stratum, or create a new
object. Symlinks, immutable state, missing parents, and read-only strata can
change that answer. The attempted operation still performs its own checks.

## Review local overrides

Use `sweep` on a mount to find objects in its create stratum, then `diff` on a
selected override before deciding whether to retain it. A gap exists only
locally; an override has a lower counterpart; a shadowed entry is hidden by a
higher object or ancestor. Directory overrides can still merge contents.

Deleting a local object can reveal the lower object because StrataFS does not
use whiteouts. The `+ro` flag prevents modification through the view, not all
direct writes to the backing tree. Neither a sweep result nor a routing report
is permission to remove the listed paths.

## Diagnose a security or copy-up failure

StrataFS requires stored descriptors under its fixed deny-missing contract.
A lower mount's synthesis does not repair a missing stored descriptor for the
view. Copy-up preserves the source's full descriptor; ordinary creation uses
inheritance. A failed preservation step fails copy-up. Do not apply overlayfs
assumptions to this implementation.

A stale copy-up after provider changes can require reopening. A successful
open does not guarantee the first write succeeds. Use the kernel's
[StrataFS documentation](~kernel/internals/stratafs) for provider identity,
copy-up, and failure semantics. `stratafs(1)` owns output states, JSON limits,
and probe-oriented exit codes.
