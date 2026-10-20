---
title: Choosing a Scheduling Niceness
type: how-to
description: >-
  Run a command with an adjusted scheduling preference.
related:
  - peios/system-and-processes/overview
---

`nice` requests a scheduling niceness for a child command. It influences how
ordinary scheduler competition is handled; it does not reserve CPU time.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
nice -n 5 peiosutils printf '%s\n' 'Background calculation'
```

## Check the result

The child prints `Background calculation` under the requested adjustment if
the scheduler accepts it.

## Keep priority separate from authority

Increasing niceness lowers scheduling preference. Requesting a higher
preference can require additional authority. Niceness does not change the
child's access token or grant access to files.

The scheduler and other workload controls still determine actual progress. See
`nice(1)` for adjustment bounds and error behavior.
