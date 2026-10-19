---
title: Using Minimal Link Operations
type: how-to
description: >-
  Create and remove one hard-link name without recursive command behavior.
related:
  - peios/files-and-directories/ln
  - peios/files-and-directories/rm
  - peios/files-and-directories/overview
---

`link` and `unlink` expose narrowly scoped name operations. `link` adds a hard
link; `unlink` removes one name.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'Draft\n' > report.txt
   ```

2. Run the next command:

   ```sh
   link report.txt second-name.txt
   ```

3. Run the next command:

   ```sh
   unlink second-name.txt
   ```

4. Read the selected file contents:

   ```sh
   cat report.txt
   ```

## Check the result

`report.txt` still contains `Draft`. Removing the second name did not remove
the remaining name or its contents.

## Understand the scope

These commands do not replace the policy choices in `ln` and `rm`. `unlink`
does not recurse through a directory tree. A hard link shares an object rather
than creating an independent copy.

Use `link(1)` and `unlink(1)` for the exact operand and failure contracts.
