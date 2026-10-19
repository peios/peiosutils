---
title: Counting Repeated Adjacent Lines
type: how-to
description: >-
  Group matching lines before counting how often each one occurs.
related:
  - peios/transforming-text/sort
  - peios/transforming-text/overview
---

`uniq` compares neighboring lines. Sorting first brings equal lines together
when their original positions do not matter.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'Aster\nBirch\nAster\n' > names.txt
   ```

2. Run the next command:

   ```sh
   LC_ALL=C sort names.txt | uniq -c
   ```

## Check the result

The result counts `Aster` twice and `Birch` once.

## Decide whether order matters

Without sorting, the example has no adjacent duplicates, so all three lines
remain. Sorting changes the input order. If order carries meaning, decide
whether you want to collapse consecutive repeats or count values across the
entire file.

Comparison options can ignore a prefix or letter case. They change which lines
are considered equal; they do not rewrite the surviving line.

See `uniq(1)` for the complete options and status contract.
