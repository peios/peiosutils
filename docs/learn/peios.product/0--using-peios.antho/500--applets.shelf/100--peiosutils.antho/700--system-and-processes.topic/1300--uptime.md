---
title: Reading System Uptime
type: how-to
description: >-
  Inspect elapsed runtime and load without relying on a legacy session file.
related:
  - peios/system-and-processes/date
  - peios/system-and-processes/overview
---

`uptime` summarizes how long the system has been running and its load. Load is
a measure of queued or active work, not a percentage of CPU use.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
uptime
```

## Check the result

The output includes an elapsed uptime and load information for the running
system.

## Keep session state separate

The file-operand interface for a legacy session database is deferred in this
implementation. It is not a supported way to obtain Peios session information.
Use the session-management tools for that task.

A load value needs context, including available processors and the kind of
work waiting. `uptime(1)` describes the supported output forms.
