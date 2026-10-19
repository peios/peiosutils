---
title: readlink
type: reference
description: Print the target of a symbolic link, or canonicalise a path by resolving every link in it.
related:
  - peios/listing-and-paths/realpath
  - peios/listing-and-paths/overview
---

`readlink` prints where a symbolic link points.

```
readlink [options] file...
```

A symbolic link is a file whose contents are another path. `readlink` reads that path and prints it:

```
$ readlink /home/jack/work
/data/projects/jack
```

By default `readlink` resolves exactly one level: it prints the link's immediate target, whether or not that target is itself a link, and whether or not it exists. If the named file is not a symbolic link, `readlink` prints nothing and reports failure.

## Canonicalising a whole path

The canonicalise options change `readlink` from "read this one link" into "resolve this entire path to its real, absolute form" — following every symbolic link in every component, collapsing `.` and `..`. The three differ only in how strict they are about components existing:

| Option | Resolves | Existence requirement |
|---|---|---|
| `-f`, `--canonicalize` | Every link in the path. | Every component except the last must exist. |
| `-e`, `--canonicalize-existing` | Every link in the path. | Every component must exist. |
| `-m`, `--canonicalize-missing` | Every link in the path. | No component need exist. |

With any of these, `readlink` succeeds on an ordinary (non-link) file too — it simply returns the canonical path. [`realpath`](~peios/listing-and-paths/realpath) is the dedicated command for this canonicalising job and has more options for it; the canonicalise flags here exist so `readlink` can do it without a second tool.

## Output and error control

| Option | Effect |
|---|---|
| `-n`, `--no-newline` | Do not print the trailing newline. Ignored, with a warning, when more than one file is given. |
| `-z`, `--zero` | End each output line with a NUL character instead of a newline. |
| `-q`, `--quiet` / `-s`, `--silent` | Suppress most error messages. This is the default. |
| `-v`, `--verbose` | Report error messages that the quiet default would suppress. |

When `POSIXLY_CORRECT` is set, `readlink` reports errors even with `-q` or `-s`.

With several operands, the current implementation stops at the first failure. Output already printed is retained, but later operands are not examined. Treat a failed invocation as incomplete.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every named file was resolved and printed. |
| `1` | In default mode, a file was not a symbolic link; in canonicalising mode, a path could not be resolved under the chosen strictness. |
