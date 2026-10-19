---
title: Creating a Directory Tree
type: how-to
description: >-
  Create missing parent directories and understand inherited security.
related:
  - peios/files-and-directories/mkfifo
  - peios/files-and-directories/mknod
  - peios/files-and-directories/touch
---

`mkdir` creates directories. A new directory normally inherits security from
its parent, so the destination location matters as well as the name.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example directory:

   ```sh
   mkdir -p reports/2026/drafts
   ```

2. Run the next command:

   ```sh
   ls -d reports/2026/drafts
   ```

## Check the result

The listing prints `reports/2026/drafts`. Repeating `mkdir -p` succeeds if the
path is already a directory.

## Choose security before sharing the directory

Peios creation options can request an owner, group, integrity label, or
explicit security descriptor. These options also exist on `mkfifo`, `mknod`,
and `touch` when they create objects.

A complete descriptor and the individual adjustments are alternative
interfaces. Do not assume that choosing a different owner alone gives that
owner the access you intended; inspect the resulting descriptor with `sd`.

Creation and applying requested security are separate operations. Failure
cleanup attempts to remove the object created by the command; do not treat
that sequence as an atomic security transaction. `mkdir(1)` describes the
creation flags and their combinations.
