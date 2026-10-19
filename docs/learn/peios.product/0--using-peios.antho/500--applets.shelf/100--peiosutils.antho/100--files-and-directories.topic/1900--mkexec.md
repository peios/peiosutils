---
title: Executable Marks and Access
type: concept
description: >-
  Distinguish marking a file executable from granting permission to run it.
related:
  - peios/files-and-directories/sd
  - peios/files-and-directories/overview
---

`mkexec` marks a regular file as executable. The mark identifies intended
executable content; it is separate from the security descriptor that
authorizes a process to execute that content.

## Two different questions

A file can have an executable mark while its descriptor denies execution to a
particular token. Conversely, an access rule does not identify random data as
an intended program. Keep the content decision and the access decision
separate.

The mark does not grant privileges, change the user's identity, or create a
trusted signature. Inspect a downloaded program and its provenance before
making it eligible to execute.

## Select the actual file

`mkexec` accepts regular files and operates on their executable metadata. It
is not a recursive permission editor. Use `sd` when the intended change is who
can access the file, and use the packaging workflow when executable metadata
should be reproducible across installations.

`mkexec(1)` describes accepted operands and errors.
