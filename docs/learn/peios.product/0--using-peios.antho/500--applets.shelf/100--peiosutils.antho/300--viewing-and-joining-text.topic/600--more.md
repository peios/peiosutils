---
title: Paging through Text
type: how-to
description: >-
  Read a long file one screen at a time in a terminal.
related:
  - peios/viewing-and-joining-text/cat
  - peios/viewing-and-joining-text/overview
---

`more` pauses between screens of text so that you can read a long file without
losing its beginning to terminal scrolling.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'Opening\nDetails\nConclusion\n' > report.txt
   ```

2. Run the next command:

   ```sh
   more report.txt
   ```

## Check the result

The three report lines appear in the pager. Press <kbd>Q</kbd> to close it. A
longer file can be read one screen at a time.

## Move through a longer file

Press <kbd>Space</kbd> for another screen, <kbd>Enter</kbd> for another line,
and <kbd>Q</kbd> to stop. Search with `/` followed by the text to find.

When output is redirected rather than displayed on a terminal, paging is not
useful. Use `cat` to copy a file into another command. The pager is for
reading, not for editing or transforming the source.

See `more(1)` for the complete options and status contract.
