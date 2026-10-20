---
title: Evaluating a Small Expression
type: how-to
description: >-
  Evaluate integer arithmetic while checking the result status correctly.
related:
  - peios/output-and-evaluation/test
  - peios/output-and-evaluation/factor
  - peios/output-and-evaluation/overview
---

`expr` evaluates a small expression passed as separate arguments. It can
perform integer arithmetic and several string operations.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
expr 7 + 5
```

## Check the result

The output is `12`.

## Separate shell syntax from expression syntax

Quote operators that the shell would interpret, such as `*` or parentheses.
Each expression token is its own argument; the shell does not parse the
arithmetic for `expr`.

A zero or empty result has a different status from a true, nonzero result. An
expression error has another status. In a script, do not treat every nonzero
status as the same kind of failure. `expr(1)` defines the distinctions.
