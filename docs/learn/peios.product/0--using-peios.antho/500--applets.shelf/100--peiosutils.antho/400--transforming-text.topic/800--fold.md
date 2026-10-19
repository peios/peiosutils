---
title: Wrapping Long Lines
type: how-to
description: >-
  Break long physical lines for display without merging paragraphs.
related:
  - peios/transforming-text/fmt
  - peios/transforming-text/overview
---

`fold` wraps each input line at a chosen width. Unlike `fmt`, it does not join
neighboring lines into a paragraph.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
printf 'Aster Birch Cedar\n' | fold -s -w 12
```

## Check the result

The text wraps at spaces where possible, keeping each word intact. The output
contains the same words in the same order.

## Choose display columns or bytes

Tabs, backspaces, and carriage returns affect column counting. A byte count is
a different measurement and can split an encoded character.

Wrapping changes the data sent to the next command. Use it for a display copy,
not for a format whose records must remain on single lines.

See `fold(1)` for the complete options and status contract.
