---
title: Checking a Proposed Path Name
type: how-to
description: >-
  Check path spelling and limits without creating a file.
related:
  - peios/listing-and-paths/overview
---

`pathchk` checks whether a path meets naming constraints. It helps catch an
unsuitable name before a later operation attempts to create it.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
pathchk reports/draft.txt
```

## Check the result

A valid name produces no output and exits successfully. The command does not
create `reports` or `draft.txt`.

## Know what a check can promise

Default checks use applicable filesystem limits and inspect searchable leading
directories where required. Portability modes use stricter naming rules
intended for exchange with other environments.

Success is not a reservation of the name or a guarantee that a write will
succeed. Access, free space, and concurrent changes still affect the actual
creation operation.

See `pathchk(1)` for the complete options and status contract.
