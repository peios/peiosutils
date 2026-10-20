---
title: Splitting Text at a Matching Line
type: how-to
description: >-
  Separate a preface from a section using a pattern boundary.
related:
  - peios/viewing-and-joining-text/split
  - peios/viewing-and-joining-text/overview
---

`csplit` separates text at line numbers or pattern matches. Use it when a
section heading, rather than a fixed file size, marks the boundary.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'Preface\nSection\nDetails\n' > report.txt
   ```

2. Run the next command:

   ```sh
   csplit report.txt '/Section/'
   ```

3. Read the selected file contents:

   ```sh
   cat xx00
   ```

4. Read the selected file contents:

   ```sh
   cat xx01
   ```

## Check the result

`xx00` contains `Preface`. `xx01` begins with `Section` and contains
`Details`. The command also reports each piece's byte count.

## Choose a disposable output location

Default output names begin with `xx`. Existing files with those names can be
overwritten. Use a fresh directory or choose a distinct prefix.

A pattern that cannot be found is an error. By default, failure removes the
pieces produced by that invocation. Keeping partial pieces is an explicit
option, not evidence that all requested sections were found.

See `csplit(1)` for the complete options and status contract.
