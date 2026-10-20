---
title: Reading a Symbolic Link
type: how-to
description: >-
  Read a link target without mistaking it for a resolved filesystem path.
related:
  - peios/listing-and-paths/realpath
  - peios/listing-and-paths/overview
---

`readlink` prints the text stored in a symbolic link. That text can be
relative and can name a target that does not exist.

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
   ln -s report.txt latest
   ```

3. Run the next command:

   ```sh
   readlink latest
   ```

## Check the result

The output is `report.txt`, the exact target stored in `latest`.

## Resolve only when the task needs it

A relative target is interpreted from the directory containing the link, not
automatically from your working directory. Reading the target does not prove
that it exists.

Use `realpath -e latest` when you need an existing, fully resolved path. An
ordinary file is not a symbolic link, so plain `readlink` fails for it. That
failure does not establish that the file is missing.

See `readlink(1)` for the complete options and status contract.
