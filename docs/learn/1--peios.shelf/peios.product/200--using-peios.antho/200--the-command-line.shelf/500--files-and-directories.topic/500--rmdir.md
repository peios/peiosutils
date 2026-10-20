---
title: Removing an Empty Directory
type: how-to
description: >-
  Remove a directory only after its contents have been dealt with.
related:
  - peios/files-and-directories/rm
  - peios/files-and-directories/overview
---

`rmdir` removes empty directories. Its refusal to remove a nonempty directory
is useful when you do not want accidental recursive deletion.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example directory:

   ```sh
   mkdir empty-directory
   ```

2. Run the next command:

   ```sh
   rmdir empty-directory
   ```

## Check the result

The directory is absent after the second command. No success message is
printed by default.

## Let a nonempty directory stop the operation

An error can mean that a directory still contains entries, including hidden
ones. Inspect those entries before deciding whether to remove them.

Parent-removal mode also attempts to remove empty ancestors. Use that mode
only when removing the entire empty path is part of the task. See `rmdir(1)`
for its handling of nonempty directories and parent paths.
