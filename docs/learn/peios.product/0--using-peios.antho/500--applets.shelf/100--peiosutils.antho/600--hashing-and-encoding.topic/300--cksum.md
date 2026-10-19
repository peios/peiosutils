---
title: Selecting a Checksum Algorithm
type: how-to
description: >-
  Choose an explicit digest instead of depending on the default checksum.
related:
  - peios/hashing-and-encoding/checksum-commands
  - peios/hashing-and-encoding/overview
---

`cksum` supports several algorithms behind one interface. Selecting the
algorithm explicitly makes the meaning of a saved result clearer.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'Release notes\n' > release.txt
   ```

2. Run the next command:

   ```sh
   cksum -a sha256 release.txt
   ```

## Check the result

The output identifies SHA-256 and records a digest for `release.txt`.

## Match the verification format

A checksum algorithm and its text output format are separate choices.
Verification expects a supported digest-list syntax. Preserve the algorithm
name and filename information when exchanging a list.

The default cyclic redundancy check is useful for detecting accidental
changes. It is not an authenticity mechanism. A cryptographic digest still
needs a trustworthy expected value. See `cksum(1)` for algorithm selection and
check-mode compatibility.
