---
title: Sizing Parallel Work
type: how-to
description: >-
  Read the processor count available to the current process.
related:
  - peios/system-and-processes/overview
---

`nproc` reports a processor count useful for choosing parallel work. The
available count can differ from the total number installed in the machine.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
nproc
```

## Check the result

The output is a positive integer representing the count available under the
command's rules.

## Leave room for other resources

CPU availability is only one constraint. Memory, storage bandwidth, and other
jobs can make a smaller worker count more useful.

Affinity and process settings can restrict the available count. The
all-processors option asks a different question. See `nproc(1)` before using
the result as an automatic build-parallelism setting.
