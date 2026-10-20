---
title: Reading an Environment Variable
type: how-to
description: >-
  Inspect one variable without dumping unrelated environment entries.
related:
  - peios/output-and-evaluation/env
  - peios/output-and-evaluation/overview
---

`printenv` prints environment values. Naming the variable keeps the output
focused and avoids disclosing unrelated entries.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
env PEIOS_EXAMPLE=ready peiosutils printenv PEIOS_EXAMPLE
```

## Check the result

The output is `ready`.

## Distinguish empty from missing

An empty value is still a defined variable. Check the exit status to tell
whether a requested name was found rather than using visible output alone.

A shell variable is not necessarily exported to child processes. If a value
exists only inside your shell, `printenv` cannot see it until it is part of
the environment.

See `printenv(1)` for the complete options and status contract.
