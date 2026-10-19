---
title: Inspecting File Metadata
type: how-to
description: >-
  Read a file size and distinguish file metadata from authorization.
related:
  - peios/listing-and-paths/ls
  - peios/listing-and-paths/overview
---

`stat` shows metadata for a file or filesystem. Use it when you need a
specific property rather than a formatted directory listing.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'Draft\n' > report.txt
   ```

2. Run the next command:

   ```sh
   stat -c '%n: %s bytes' report.txt
   ```

## Check the result

The output is `report.txt: 6 bytes`.

## Interpret compatibility fields carefully

Peios authorizes access using security descriptors and tokens. A mode, user
number, or group number printed for compatibility does not describe all access
rules. Use `sd` when your question concerns permission.

A symbolic link has metadata of its own. Decide whether you want the link or
its target before selecting dereference behavior.

A metadata query is a momentary observation. A size can change before a later
read, so a program enforcing limits must check the file it opens and bound the
read itself. Custom formats and filesystem fields belong in `stat(1)`.
