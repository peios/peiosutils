---
title: Resolving a Filesystem Path
type: how-to
description: >-
  Select whether the final path component and its parents must exist.
related:
  - peios/listing-and-paths/readlink
  - peios/listing-and-paths/overview
---

`realpath` produces an absolute path while resolving symbolic links and path
components. Existence requirements are part of choosing the right resolution
mode.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example directory:

   ```sh
   mkdir reports
   ```

2. Create the example input:

   ```sh
   printf 'Draft\n' > reports/draft.txt
   ```

3. Run the next command:

   ```sh
   realpath -e reports/draft.txt
   ```

4. Run the next command:

   ```sh
   realpath reports/planned.txt
   ```

## Check the result

The first result names the existing draft. The second names a planned file
under the existing `reports` directory without creating it.

## Choose an existence requirement

Use `-e` when every component must exist. Default resolution allows the final
component to be absent but requires existing parents. Use `-m` to allow
missing parents too; the result is a planned spelling, not evidence that the
location can be opened.

Quiet mode suppresses diagnostics while retaining a failing exit status. Check
that status before consuming output. Resolving a name is not an access check,
a file creation, or protection against concurrent replacement.

Logical and physical treatment of symbolic links can differ around `..`. See
`realpath(1)` for those choices and relative-output modes.
