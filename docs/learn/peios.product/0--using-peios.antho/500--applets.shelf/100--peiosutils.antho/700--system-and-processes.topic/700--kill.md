---
title: Choosing a Process Signal
type: how-to
description: >-
  Inspect signal names before requesting that a process change state.
related:
  - peios/system-and-processes/timeout
  - peios/system-and-processes/overview
---

`kill` sends signals to a selected process or process group. A signal is a
request with a specific meaning, not always an immediate forced stop.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
peiosutils kill -l
```

## Check the result

The output lists recognized signal names. This invocation does not signal any
process.

## Identify the process before signaling it

Process numbers can be reused after a process exits. Check the target through
the appropriate process-management interface before acting.

A normal termination signal gives a cooperating program an opportunity to
clean up. A forced kill does not. Permission to signal a target is an access
decision; knowing its number grants no authority.

A shell often supplies its own `kill`. Use `kill(1)` for this implementation's
process-group syntax and status rules.
