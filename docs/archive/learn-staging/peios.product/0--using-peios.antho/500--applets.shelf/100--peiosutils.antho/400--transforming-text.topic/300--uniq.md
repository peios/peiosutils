---
title: uniq
type: reference
description: Collapse or report adjacent repeated lines in a file.
related:
  - peios/transforming-text/sort
  - peios/transforming-text/overview
---

`uniq` deals with repeated lines: it can collapse a run of identical lines into one, count them, or print only the ones that repeated.

```
uniq [options] [input [output]]
```

```
$ uniq events.txt
```

`input` and `output` may be given as operands; with neither, `uniq` reads standard input and writes standard output.

## uniq only sees adjacent duplicates

This is the one thing to know about `uniq`: it only compares **each line with the one before it**. It collapses *adjacent* repeats. Duplicate lines scattered through a file, with other lines between them, are not caught.

So `uniq` is almost always paired with [`sort`](~peios/transforming-text/sort), which brings every copy of a line together first:

```
$ sort events.txt | uniq
```

If collapsing duplicates is all you need, `sort -u` does both jobs in one command. Use `uniq` itself when you want what `sort -u` cannot do — counting repeats, or printing only the duplicated lines.

## What to output

By default `uniq` prints every line, with each run of adjacent duplicates reduced to a single copy. These options change that:

| Option | Effect |
|---|---|
| `-c`, `--count` | Prefix each line with the number of times it occurred. |
| `-d`, `--repeated` | Print only lines that *were* repeated — one copy of each. |
| `-D` | Print *all* copies of every repeated line, not just one. |
| `-u`, `--unique` | Print only lines that were *not* repeated. |
| `--group[=WHICH]` | Print every line, with runs separated by a blank line. `WHICH` is `separate`, `prepend`, `append`, or `both`. |

## What counts as "the same"

| Option | Effect |
|---|---|
| `-i`, `--ignore-case` | Treat lines differing only in letter case as the same. |
| `-f`, `--skip-fields=N` | Ignore the first `N` fields when comparing. |
| `-s`, `--skip-chars=N` | Ignore the first `N` characters when comparing. |
| `-w`, `--check-chars=N` | Compare at most `N` characters of each line. |

## Other options

| Option | Effect |
|---|---|
| `-z`, `--zero-terminated` | Treat the NUL character as the line delimiter. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A file could not be read or written. |
