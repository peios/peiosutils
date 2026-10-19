---
title: Pausing a Script
type: how-to
description: >-
  Wait for a duration while keeping success distinct from useful work.
related:
  - peios/system-and-processes/timeout
  - peios/system-and-processes/overview
---

`sleep` delays the next shell operation. It is useful for pacing a script when
a delay is intended, rather than guessing when another service will be ready.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Run the next command:

   ```sh
   sleep 0.1
   ```

2. Run the next command:

   ```sh
   printf '%s\n' 'Delay complete'
   ```

## Check the result

The message appears after a short delay. Scheduling can make the elapsed time
longer than the requested duration.

## Prefer a readiness signal when one exists

A delay does not prove that a service started, a file finished copying, or a
network connection became available. Use an explicit readiness check for those
conditions.

`sleep(1)` defines duration suffixes and how multiple durations combine.
