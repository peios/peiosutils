---
title: Replacing Spaces with Tabs
type: how-to
description: >-
  Compress aligned spacing using an agreed set of tab stops.
related:
  - peios/transforming-text/expand
  - peios/transforming-text/overview
---

`unexpand` replaces suitable runs of spaces with tabs. The result keeps its
alignment only when the reader uses the same tab stops.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf '    A\n' > spaces.txt
   ```

2. Run the next command:

   ```sh
   unexpand -t 4 spaces.txt | cat -T
   ```

## Check the result

The visible representation begins with `^I`, showing the inserted tab,
followed by `A`.

## Agree on the tab width

The default operation converts leading spacing. Converting spacing inside a
line is a separate choice. Specify tab stops that match the tool or editor
that will read the result.

This conversion does not preserve the original bytes and is not a general
compression format. Keep space-only text when its consumers do not share a tab
convention.

See `unexpand(1)` for the complete options and status contract.
