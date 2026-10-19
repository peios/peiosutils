---
title: Replacing Tabs with Spaces
type: how-to
description: >-
  Turn tab stops into spaces for a predictable text layout.
related:
  - peios/transforming-text/unexpand
  - peios/transforming-text/overview
---

`expand` replaces tabs with the number of spaces needed to reach a tab stop.
It is useful when the reader's tab settings should not change a file's visible
alignment.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'A\tB\n' > tabs.txt
   ```

2. Run the next command:

   ```sh
   expand -t 4 tabs.txt
   ```

## Check the result

Three spaces separate `A` from `B`, placing `B` at the next four-column tab
stop.

## Understand the limit of conversion

The number of spaces depends on the current column, not only on the tab
character. The default tab stops are eight columns apart.

Expansion changes the bytes of the output. Keep the original if tabs are
meaningful to the file format, as in some build instructions. Redirect to a
different filename when saving a converted version.

See `expand(1)` for the complete options and status contract.
