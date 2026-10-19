---
title: Adjusting a Program's Output Buffering
type: how-to
description: >-
  Choose when a compatible child program flushes its standard streams.
related:
  - peios/system-and-processes/overview
---

`stdbuf` requests different buffering for a child program's standard streams.
It can make progress lines appear sooner when buffering delays a pipeline.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
stdbuf -oL peiosutils printf '%s\n' 'Progress line'
```

## Check the result

The child prints `Progress line`. Whether the buffering request changes its
behavior depends on the child's I/O implementation.

## Understand what the wrapper can control

The command uses a preload library to adjust compatible C standard-I/O
streams. A program that bypasses those streams, changes buffering itself, or
cannot load that library can ignore the request.

`stdbuf` cannot force a program to produce output it has not written. Choose a
program's own progress or flushing option when one exists. See `stdbuf(1)` for
supported stream settings and invocation failures.
