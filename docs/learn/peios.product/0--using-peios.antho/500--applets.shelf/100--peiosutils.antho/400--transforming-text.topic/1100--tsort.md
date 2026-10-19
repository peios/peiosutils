---
title: Ordering Dependencies
type: how-to
description: >-
  Turn prerequisite pairs into an order that respects those dependencies.
related:
  - peios/transforming-text/sort
  - peios/transforming-text/overview
---

`tsort` reads pairs where the first item must precede the second. It finds an
ordering that satisfies those relationships when no cycle exists.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
printf 'prepare build\nbuild test\n' | tsort
```

## Check the result

The output is `prepare`, `build`, and `test`, on separate lines.

## Treat cycles as errors

A cycle asks an item to precede itself through other items. No valid total
order can satisfy that request. Check the exit status rather than accepting
partial output from a failed ordering.

Independent items can have more than one valid order. Do not treat their
relative placement as an extra dependency that was never in the input.

See `tsort(1)` for the complete options and status contract.
