---
title: Identifying the Kernel and Operating System
type: how-to
description: >-
  Read system identifiers without confusing the kernel name with the product
  name.
related:
  - peios/system-and-processes/arch
  - peios/system-and-processes/hostname
  - peios/system-and-processes/overview
---

`uname` reports identifiers for the running kernel and operating system. These
fields answer different questions and should not be treated as interchangeable
version strings.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Run the next command:

   ```sh
   uname -s
   ```

2. Run the next command:

   ```sh
   uname -o
   ```

## Check the result

The first command reports the kernel name. This Peiosutils implementation
reports `Peios` for the operating-system field.

## Select the field your program needs

The machine field describes the reported hardware architecture, while the
kernel release describes the running kernel. Neither identifies the exact
versions of every installed component.

Use package metadata for component versions and explicit feature checks for
capabilities. `uname(1)` lists each field and the order of combined output.
