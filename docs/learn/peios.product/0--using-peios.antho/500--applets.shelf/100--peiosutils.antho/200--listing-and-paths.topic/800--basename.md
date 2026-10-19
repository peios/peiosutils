---
title: Extracting a File Name
type: how-to
description: >-
  Take the final component of a path without accessing the filesystem.
related:
  - peios/listing-and-paths/dirname
  - peios/listing-and-paths/overview
---

`basename` extracts the final component of a path. It is a text operation, so
the named file does not need to exist.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Run the next command:

   ```sh
   basename reports/draft.txt
   ```

2. Run the next command:

   ```sh
   basename reports/draft.txt .txt
   ```

## Check the result

The first command prints `draft.txt`. The second prints `draft`, with the
matching suffix removed.

## Keep names distinct from files

Removing a suffix is a literal operation, not file-type detection. A name
ending in `.txt` can contain any bytes.

For multiple names, use the command's multiple-operand mode. Choose
zero-terminated output when a consumer must distinguish names containing
newlines. `basename(1)` defines trailing-separator and root-path handling.
