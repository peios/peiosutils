---
title: Bounding a Command Run
type: how-to
description: >-
  Stop waiting for a command after a selected duration.
related:
  - peios/system-and-processes/kill
  - peios/system-and-processes/sleep
  - peios/system-and-processes/overview
---

`timeout` runs a command with a time limit. It sends a signal when the limit
expires, so the child must still respond to that signal.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Run the next command:

   ```sh
   timeout 1 peiosutils sleep 0.1
   ```

2. Run the next command:

   ```sh
   printf 'Status: %s\n' "$?"
   ```

## Check the result

The child finishes before the limit and the printed status is zero.

## Distinguish a deadline from cleanup

An initial signal can be ignored or handled slowly. A later forced-stop option
can bound that grace period, but cannot guarantee that the child's work is
complete or recoverable.

A timed-out command can leave partial output or external changes. Design the
operation's recovery separately. `timeout(1)` defines timeout statuses, signal
choices, and foreground process-group behavior.
