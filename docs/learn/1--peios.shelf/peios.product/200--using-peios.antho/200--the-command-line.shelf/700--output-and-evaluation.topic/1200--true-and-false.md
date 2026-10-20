---
title: Producing a Command Status
type: how-to
description: >-
  Use predictable success and failure commands in shell control flow.
related:
  - peios/output-and-evaluation/test
  - peios/output-and-evaluation/overview
---

`true` succeeds and `false` fails. Their purpose is the exit status, not
printed output.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Evaluate the condition and run its matching branch:

   ```sh
   if peiosutils true; then
       peiosutils printf '%s\n' 'Success branch'
   fi
   ```

2. Evaluate the condition and run its matching branch:

   ```sh
   if peiosutils false; then
       peiosutils printf '%s\n' 'Unexpected branch'
   fi
   ```

## Check the result

Only `Success branch` appears.

## Do not hide an operational error

An unconditional successful fallback can be useful when failure is expected
and harmless. It can also conceal a command that did not complete its task.
Handle the specific failure you expect before replacing its status with
`true`.

See `true(1)` and `false(1)` for their status contracts.
