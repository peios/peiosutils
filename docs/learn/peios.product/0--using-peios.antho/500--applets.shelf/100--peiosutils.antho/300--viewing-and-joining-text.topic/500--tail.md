---
title: Reading the End of a File
type: how-to
description: >-
  Inspect the final records and choose how to follow a growing log.
related:
  - peios/viewing-and-joining-text/head
  - peios/viewing-and-joining-text/overview
---

`tail` shows the end of a file. It can also wait for appended data, which
helps you observe a running program's log.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'starting\nworking\nfinished\n' > progress.log
   ```

2. Run the next command:

   ```sh
   tail -n 2 progress.log
   ```

## Check the result

The output contains `working` and `finished`.

## Follow the writer or the name

Use `tail -f progress.log` to follow the opened file. If another program
renames that file and creates a replacement, the opened file and the name
refer to different objects.

Use `tail -F progress.log` when you want the name to keep selecting the log,
including after replacement. This form retries when the name is absent. Stop
following with <kbd>Ctrl</kbd>+<kbd>C</kbd>.

Following a log is an observation, not a delivery guarantee. Keep the log
itself when every record matters.

See `tail(1)` for the complete options and status contract.
