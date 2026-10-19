---
title: Combining Corresponding Lines
type: how-to
description: >-
  Put the first line of one file beside the first line of another.
related:
  - peios/viewing-and-joining-text/cut
  - peios/viewing-and-joining-text/join
  - peios/viewing-and-joining-text/overview
---

`paste` combines lines by position. Use it when two files already have the
same row order and you want their corresponding lines side by side.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'Aster\nBirch\n' > names.txt
   ```

2. Create the example input:

   ```sh
   printf '3\n5\n' > counts.txt
   ```

3. Run the next command:

   ```sh
   paste names.txt counts.txt
   ```

## Check the result

Each name appears beside its count, separated by a tab.

## Position is not identity

`paste` does not compare names or keys. A missing line shifts the pairing, and
a shorter file contributes empty fields after it ends. Use `join` when
matching keys should decide which records belong together.

The delimiter can be changed for display. Choosing a delimiter does not escape
that character when it already occurs inside an input line.

See `paste(1)` for the complete options and status contract.
