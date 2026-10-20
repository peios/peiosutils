---
title: Changing a Process's Root View
type: concept
description: >-
  Separate filesystem path rebasing from a security sandbox.
related:
  - peios/system-and-processes/overview
---

`chroot` starts a command with a different filesystem root. Absolute paths in
that command are then interpreted relative to the selected root view.

## A usable root contains dependencies

The command, its interpreter or loader, and required libraries must be
available inside the selected tree. A program file alone is often insufficient
to start it.

The operation requires the relevant authority. Changing a root view does not
automatically discard privileges, replace the token, isolate network access,
or close already-open descriptors.

## Use isolation designed for the threat

A changed root is a path-lookup mechanism, not a complete containment
boundary. Design the token, handles, process environment, and other isolation
mechanisms together when running untrusted code.

`chroot(1)` defines the supported invocation and execution behavior.
