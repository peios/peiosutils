---
title: Choosing How to Resolve a Path
type: concept
description: >-
  Choose between link text, an existing object, and a planned output path.
related:
  - peios/listing-and-paths/overview
  - peios/listing-and-paths/realpath
  - peios/listing-and-paths/readlink
---

A path can be a name stored in a link, a route to an existing object, or a
plan for an output that does not exist. Choose the operation that answers your
actual question before normalizing the spelling.

## Read a link or resolve a route

`readlink` reads the immediate target stored in a symbolic link. The target
can be relative or absent. It does not provide an absolute path or prove that
a later open will succeed.

`realpath` resolves the route through the filesystem. It follows links and
normalizes path components, subject to the selected existence requirements. It
also accepts ordinary files, which plain `readlink` does not.

## Decide what must exist

For an input that must already exist, select `realpath -e`. Every component
must resolve. For a new output beneath existing parents, default resolution
allows the final component to be absent.

For a planned tree with missing parents, `realpath -m` permits missing
components too. That produces a planned spelling without creating the tree or
verifying permission to create it.

Check the command status before using the output. Quiet mode suppresses error
messages, not a failing status. Resolution cannot reserve a name against
concurrent replacement.

## Extract text when that is sufficient

`basename` and `dirname` split path text without consulting the filesystem.
Use them when the task is to construct related names rather than validate an
existing object. `pathchk` checks naming constraints, which is another
distinct question.

The manuals `readlink(1)`, `realpath(1)`, `basename(1)`, `dirname(1)`, and
`pathchk(1)` own their complete path-handling rules.
