---
title: Checking a File against a Recorded Digest
type: how-to
description: >-
  Create and verify a checksum list while understanding what it proves.
related:
  - peios/hashing-and-encoding/cksum
  - peios/hashing-and-encoding/overview
---

A checksum compares file bytes with a previously recorded digest. It can show
that bytes changed, but the checksum list must come from a trusted source if
you use it to judge authenticity.

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
   sha256sum release.txt > checksums.txt
   ```

3. Run the next command:

   ```sh
   sha256sum -c checksums.txt
   ```

## Check the result

The verification reports `release.txt: OK`.

## Trust the expected value

An attacker who can replace both a file and its checksum can make this check
succeed. Obtain the expected digest through an authenticated channel, or
verify a signature under a trusted key.

Use a modern cryptographic digest for content verification. Older sums remain
useful for compatibility, but collision-resistant identity is a different
requirement from detecting an accidental transmission error.

The named checksum utilities share the calculation-and-check workflow. See
`sha256sum(1)` and the chosen command's manual for binary/text markers,
filename escaping, and failure statuses.
