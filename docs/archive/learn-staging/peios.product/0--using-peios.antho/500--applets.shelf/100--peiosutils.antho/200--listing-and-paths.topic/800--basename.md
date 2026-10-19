---
title: basename
type: reference
description: Strip the directory part off a path, leaving just the final name — and optionally remove a trailing suffix.
related:
  - peios/listing-and-paths/dirname
  - peios/listing-and-paths/overview
---

`basename` takes a path and prints just its final component — the file name, with all the leading directories removed.

```
basename name [suffix]
basename [options] name...
```

```
$ basename /home/jack/projects/notes.txt
notes.txt
```

It is a pure text operation: `basename` never looks at the file system. It works on the string you give it, so the path need not exist.

## Removing a suffix

Give a second argument and `basename` also strips that suffix from the end of the name — handy for turning a file name into a bare stem:

```
$ basename /home/jack/projects/notes.txt .txt
notes
```

The suffix is only removed if it is actually there, and never if it would leave an empty string.

## Options

`basename` takes one name by default. The options let it process several at once.

| Option | Effect |
|---|---|
| `-a`, `--multiple` | Treat every argument as a name to process, instead of treating the second argument as a suffix. Required to pass more than one name. |
| `-s`, `--suffix=SUFFIX` | Remove `SUFFIX` from each name. Implies `-a`, so it applies to every name given. |
| `-z`, `--zero` | End each output line with a NUL character instead of a newline. |

```
$ basename -s .txt notes.txt readme.txt changelog.txt
notes
readme
changelog
```

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A usage error — a missing or extra operand. |
