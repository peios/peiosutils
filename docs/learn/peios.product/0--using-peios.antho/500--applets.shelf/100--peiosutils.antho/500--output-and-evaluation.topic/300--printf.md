---
title: Printing Formatted Values
type: how-to
description: >-
  Keep a fixed format separate from the values inserted into it.
related:
  - peios/output-and-evaluation/echo
  - peios/output-and-evaluation/overview
---

`printf` formats values using a format string. It provides predictable
newlines and field formatting when a plain message is not enough.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
peiosutils printf '%s: %d\n' Aster 3
```

## Check the result

The output is `Aster: 3`, followed by a newline.

## Keep the format under your control

Use a literal format and pass data as later arguments. Data placed in the
format can be interpreted as conversion or escape syntax.

The format repeats if more arguments remain. Missing values receive the
conversion's default value. When the record shape matters, validate the number
and kind of values before printing.

A shell can provide its own `printf`. The explicit multicall form selects the
Peiosutils command. See `printf(1)` for conversions and escapes.
