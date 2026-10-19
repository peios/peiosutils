---
title: Creating a Temporary Working Directory
type: how-to
description: >-
  Create a unique location instead of guessing a temporary name.
related:
  - peios/files-and-directories/touch
  - peios/files-and-directories/mkdir
  - peios/files-and-directories/overview
---

`mktemp` creates a unique file or directory and prints its name. Creation is
part of the operation, so another process cannot claim a merely guessed name
before your later use.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create a unique working directory:

   ```sh
   workdir=$(mktemp -d)
   ```

2. Run the next command:

   ```sh
   printf 'Created %s\n' "$workdir"
   ```

3. Run the next command:

   ```sh
   rmdir "$workdir"
   ```

## Check the result

The printed directory exists until the final command removes it. Its name
varies between runs.

## Keep creation and use connected

Check that `mktemp` succeeded before using its output in a longer script.
Quote the resulting path and arrange cleanup for the success and failure paths
of the work it contains.

A dry-run name is not a reservation. Do not use an uncreated suggested name as
a safe substitute for exclusive creation. `mktemp(1)` defines templates,
directory selection, and cleanup responsibilities.
