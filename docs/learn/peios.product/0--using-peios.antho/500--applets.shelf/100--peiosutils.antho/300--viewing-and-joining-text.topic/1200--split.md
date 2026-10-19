---
title: Splitting a File into Pieces
type: how-to
description: >-
  Divide a text file by line count and reassemble its pieces in order.
related:
  - peios/viewing-and-joining-text/csplit
  - peios/viewing-and-joining-text/cat
  - peios/viewing-and-joining-text/overview
---

`split` writes consecutive portions of a file into separate files. This helps
you handle an input in smaller pieces while retaining its order.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'one\ntwo\nthree\nfour\nfive\n' > original.txt
   ```

2. Run the next command:

   ```sh
   split -l 2 original.txt piece-
   ```

3. Read the selected file contents:

   ```sh
   cat piece-aa piece-ab piece-ac > restored.txt
   ```

4. Read the selected file contents:

   ```sh
   cat restored.txt
   ```

## Check the result

The restored file has the same five lines as the original. The last piece
contains one line.

## Keep output names separate

Run the task in a directory without existing `piece-` files. Output names can
be replaced. The command checks whether an opened output is the input before
truncating it, but that check does not protect unrelated old pieces.

Byte limits can divide a text line or encoded character. Use line limits when
each piece must remain readable text. Reassembly needs the complete piece set
in suffix order, without unrelated files matched by a wildcard.

New pieces inherit security from their destination directory. Splitting is not
a way to preserve the original file's complete metadata.

See `split(1)` for the complete options and status contract.
