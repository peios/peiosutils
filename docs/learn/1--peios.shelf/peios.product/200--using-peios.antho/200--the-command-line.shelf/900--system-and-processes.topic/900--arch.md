---
title: Reading the Reported Machine Architecture
type: how-to
description: >-
  Inspect the machine identifier without treating it as a complete
  compatibility check.
related:
  - peios/system-and-processes/uname
  - peios/system-and-processes/overview
---

`arch` prints the reported machine architecture. The result can help orient
diagnostics, but does not describe every instruction or software interface
available to a process.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
arch
```

## Check the result

The output is the machine identifier reported by the running system.

## Check the capability you need

A machine name alone does not establish that a particular executable, library
interface, or processor extension is usable. Packages and programs need their
own compatibility checks.

`arch(1)` describes the command; `uname(1)` covers the related system fields.
