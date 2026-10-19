---
title: Understanding File-Erasure Limits
type: how-to
description: >-
  Decide whether overwriting a file reaches the data you intend to erase.
related:
  - peios/files-and-directories/rm
  - peios/files-and-directories/overview
---

`shred` overwrites a file's accessible data. Whether that destroys all
recoverable copies depends on the filesystem and storage device, not only on
the number of overwrite passes.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'Disposable sample\n' > erase-sample.txt
   ```

2. Run the next command:

   ```sh
   shred -n 1 -z erase-sample.txt
   ```

## Check the result

The file remains under its name, with overwritten contents. This example does
not establish secure erasure on your storage device.

## Account for copies outside the file

Copy-on-write filesystems, snapshots, backups, journal copies, and device wear
leveling can retain data that an overwrite does not reach. More passes over
the visible file do not remove those independent copies.

Use a storage-appropriate erasure or encryption-key lifecycle when the
requirement is that sensitive data become unrecoverable. Document which copies
that procedure covers.

Forced writing still needs authorization under the security descriptor. It is
not a bypass of Peios access checks. `shred(1)` describes overwrite and
name-removal options, but cannot guarantee a device's behavior.
