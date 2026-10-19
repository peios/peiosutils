---
title: Finding Prime Factors
type: how-to
description: >-
  Decompose a positive integer into its prime factors.
related:
  - peios/output-and-evaluation/expr
  - peios/output-and-evaluation/overview
---

`factor` prints the prime factors of each supplied integer. It is useful for
inspecting divisibility without writing a factorization routine.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
factor 84
```

## Check the result

The output is `84: 2 2 3 7`. Multiplying the factors recovers 84.

## Keep the input within the task

The command works with integers, not decimal approximations. Large inputs can
require more work than their printed length suggests.

Repeated factors are significant: the two occurrences of `2` in the example
express a factor of four. See `factor(1)` for accepted input forms.
