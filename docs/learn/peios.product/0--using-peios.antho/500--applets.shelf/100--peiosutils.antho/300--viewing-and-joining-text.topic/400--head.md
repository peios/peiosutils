---
title: Reading the Start of a File
type: how-to
description: >-
  Preview the opening lines of a file before processing the whole input.
related:
  - peios/viewing-and-joining-text/tail
  - peios/viewing-and-joining-text/overview
---

`head` shows the beginning of a file. A short preview helps you recognize a
header or inspect a sample without printing the whole file.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'name\nAster\nBirch\nCedar\n' > names.txt
   ```

2. Run the next command:

   ```sh
   head -n 2 names.txt
   ```

## Check the result

The output contains the header `name` and the first entry `Aster`.

## Count bytes or lines

The default preview is 10 lines. Use `-n` for a line count and `-c` for a byte
count. A byte limit can stop inside an encoded character, so use a line limit
for readable text.

With several files, `head` adds labels identifying each input. Those labels
become part of redirected output too. See `head(1)` before using multiple
inputs in a machine-readable pipeline.
