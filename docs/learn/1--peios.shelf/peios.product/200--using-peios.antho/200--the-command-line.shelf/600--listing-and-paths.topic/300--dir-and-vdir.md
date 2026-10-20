---
title: Choosing a Directory Listing Layout
type: how-to
description: >-
  Use fixed compact or detailed listings without depending on terminal
  detection.
related:
  - peios/listing-and-paths/ls
  - peios/listing-and-paths/overview
---

`dir` and `vdir` share the listing engine used by `ls`. They choose
predictable defaults for layout: columns for `dir`, detailed rows for `vdir`.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example directory:

   ```sh
   mkdir listings
   ```

2. Create the example input:

   ```sh
   printf 'Draft\n' > listings/report.txt
   ```

3. Run the next command:

   ```sh
   dir listings
   ```

4. Run the next command:

   ```sh
   vdir listings
   ```

## Check the result

Both commands list `report.txt`. The detailed view also includes metadata such
as its size and modification time.

## Keep layout separate from meaning

The commands change the presentation, not the entries being inspected. Most
listing controls come from `ls`, including sorting and selection.

A detailed listing is still a human-readable view. Its compatibility mode and
ownership fields do not replace a Peios security-descriptor check. See
`dir(1)`, `vdir(1)`, and `ls(1)` for the complete interfaces.
