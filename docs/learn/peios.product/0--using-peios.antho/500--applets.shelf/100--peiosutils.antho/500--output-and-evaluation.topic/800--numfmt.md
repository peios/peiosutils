---
title: Formatting Human-Readable Quantities
type: how-to
description: >-
  Convert an exact byte count into a compact display value.
related:
  - peios/output-and-evaluation/overview
---

`numfmt` converts numbers between plain and scaled forms. Use it for a
human-readable display while retaining the unrounded original for
calculations.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
printf '1024\n1048576\n' | numfmt --to=iec-i
```

## Check the result

The output is `1.0Ki` and `1.0Mi`, on separate lines.

## Match the units to the data

Binary prefixes scale by powers of 1024; decimal prefixes scale by powers of
1000. Choose the convention required by the data source rather than inferring
it from a convenient-looking suffix.

Rounding can lose information. Do not use a formatted display as an exact
storage limit or as a replacement for the original numeric value. Field
selection and padding belong in `numfmt(1)`.
