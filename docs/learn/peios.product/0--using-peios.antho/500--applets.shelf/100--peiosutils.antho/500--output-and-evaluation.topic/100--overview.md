---
title: Values, Streams, and Command Status
type: concept
description: >-
  Choose between printing data, changing a child environment, and testing a
  condition.
related:
  - peios/output-and-evaluation/echo
  - peios/output-and-evaluation/test
---

A shell command communicates through output streams and an exit status. Those
channels carry different kinds of information. Use output for data and status
for the command's defined success or condition result.

## Keep formatting separate from data

A fixed `printf` format prevents data from becoming formatting instructions.
Environment values are another kind of input, and can affect a child without
changing the parent shell.

These guides cover the common building blocks:

- [Printing formatted values](~peios/output-and-evaluation/printf) keeps data separate.
- [Running with an environment setting](~peios/output-and-evaluation/env) scopes a change.
- [Reading a variable](~peios/output-and-evaluation/printenv) avoids an environment dump.
- [Testing a condition](~peios/output-and-evaluation/test) selects a branch by status.
- [Formatting quantities](~peios/output-and-evaluation/numfmt) explains scaling.
- [Saving a stream](~peios/output-and-evaluation/tee) creates an additional output.

## Interpret status using the command's contract

A false condition is not necessarily an operational error. Some commands use
separate statuses for false results, invalid input, and I/O failures. A
pipeline also contains several command statuses, and the shell decides which
one becomes the pipeline result.

Consult the command's manual and the shell's pipeline rules before writing
error handling that treats every nonzero result alike.
