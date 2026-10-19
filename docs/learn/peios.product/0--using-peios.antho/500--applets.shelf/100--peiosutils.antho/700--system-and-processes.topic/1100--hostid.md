---
title: Understanding a Host Identifier
type: how-to
description: >-
  Recognize the limits of the short identifier printed by hostid.
related:
  - peios/system-and-processes/hostname
  - peios/system-and-processes/overview
---

`hostid` prints a short host identifier exposed by the environment. It is an
identifier for compatibility, not a cryptographic machine identity.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
hostid
```

## Check the result

The output is a hexadecimal identifier. Its value depends on the host
environment.

## Do not use the value as an authentication secret

A short identifier can repeat across systems and can be influenced by
configuration. It does not prove which machine produced a message.

Use an authenticated identity mechanism when trust depends on the machine. See
`hostid(1)` for the value's representation.
