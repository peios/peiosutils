---
title: Printing a Short Message
type: how-to
description: >-
  Print uncomplicated status text and recognize shell-specific behavior.
related:
  - peios/output-and-evaluation/printf
  - peios/output-and-evaluation/overview
---

`echo` prints its arguments separated by spaces. It is suitable for short
literal messages whose contents do not resemble options or escape syntax.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
peiosutils echo 'Build complete'
```

## Check the result

The output is `Build complete`, followed by a newline.

## Use a format for arbitrary data

Shells often provide their own `echo`, and option or escape handling can
differ. The explicit `peiosutils` invocation selects this implementation.

Use `printf '%s\n' "$value"` when printing a variable whose contents can begin
with `-` or contain backslashes. The fixed format keeps the value from being
interpreted as formatting instructions.

See `echo(1)` for the complete options and status contract.
