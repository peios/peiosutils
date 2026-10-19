---
title: pathchk
type: reference
description: Check whether path names are valid and usable before you try to create the files they name.
related:
  - peios/listing-and-paths/overview
---

`pathchk` checks whether a path name is valid and usable. It tells you, *before* you try to create a file, whether the name would be rejected — because it is too long, contains an unusable component, or could not be reached.

```
pathchk [options] name...
```

```
$ pathchk /home/jack/projects/notes.txt
$ echo $?
0
```

`pathchk` prints nothing when a name is fine; it prints a diagnostic and fails when a name is not. It is meant for scripts that build up path names and want to fail early, with a clear message, instead of discovering the problem halfway through an operation.

Like the other name commands in this topic, `pathchk` does not require the file to exist — it is checking the *name*, not looking for the file.

## What the default check covers

With no options, for each name `pathchk` checks:

- that the name is not empty;
- that no part of the name exceeds the length limits of the file systems the path would actually touch;
- that the leading directories of the name can be searched.

## Stricter checks

The options replace "valid on *this* system" with "valid across a wide range of systems" — useful when a name has to work somewhere other than where you are checking it.

| Option | Adds |
|---|---|
| `-p` | Check against a fixed, conservative set of length limits instead of the local file system's, and flag characters that are not broadly portable. |
| `-P` | Reject an empty name, and reject any component that begins with `-`. |
| `--portability` | Apply both `-p` and `-P` — the strictest check. |

A name with a leading `-` on a component is worth catching: many commands would read it as an option rather than a file name. `-P` flags exactly that class of trouble.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every name passed every requested check. |
| `1` | At least one name failed. A diagnostic naming the problem was printed. |
