---
title: Sorting Text Records
type: how-to
description: >-
  Choose between text order and numeric order before combining records.
related:
  - peios/transforming-text/uniq
  - peios/transforming-text/shuf
  - peios/transforming-text/overview
---

`sort` arranges lines according to comparison rules. Choose those rules before
using sorted output as input to another command.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf '12\n3\n20\n' > counts.txt
   ```

2. Run the next command:

   ```sh
   LC_ALL=C sort -n counts.txt
   ```

## Check the result

The output is `3`, `12`, and `20`, on separate lines. Text ordering alone
would put `12` before `3`.

## Make the comparison deliberate

Locale affects text comparisons. `LC_ALL=C` selects a stable byte-oriented
ordering for reproducible pipelines. Numeric sorting interprets numbers rather
than their leading characters.

Use a key field when a record contains both a name and a value. Apply the same
field and locale rules to later operations such as `join` or `comm`.

Redirect output to a different file, or use the command's output option. Shell
redirection to the input filename truncates that input before sorting starts.
`sort(1)` describes output handling and temporary-storage controls.
