---
title: Extracting a Parent Path
type: how-to
description: >-
  Take the directory portion of a path as text.
related:
  - peios/listing-and-paths/basename
  - peios/listing-and-paths/overview
---

`dirname` removes the final component of a path. Use it when constructing
related names, without asking the filesystem to resolve them.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Run the next command:

   ```sh
   dirname reports/draft.txt
   ```

2. Run the next command:

   ```sh
   dirname draft.txt
   ```

## Check the result

The outputs are `reports` and `.`, respectively. A name with no directory
component is relative to the working directory.

## Text does not establish access

The result can name a missing directory or contain a symbolic link. Use a
filesystem operation when you need to check whether a directory exists or can
be opened.

`dirname(1)` defines handling of trailing separators and root paths. For
programmatic use, preserve filenames containing newlines with an appropriate
record separator.
