---
title: csplit
type: reference
description: Break a file into pieces at lines chosen by line number or by a matching pattern.
related:
  - peios/viewing-and-joining-text/split
  - peios/viewing-and-joining-text/overview
---

`csplit` — "context split" — breaks a file into pieces at points *you* choose: a line number, or a line that matches a pattern. Where [`split`](~peios/viewing-and-joining-text/split) cuts a file into equal sizes, `csplit` cuts it at meaningful boundaries.

```
csplit [options] file pattern...
```

```
$ csplit report.txt '/^Chapter/' '{*}'
```

That cuts `report.txt` at every line beginning with `Chapter`, including an initial piece before the first match. That initial piece is empty if the first line matches. The pieces are written to `xx00`, `xx01`, `xx02`, … and `csplit` prints the byte size of each.

Each piece is a newly created file, and a new file needs a [security descriptor](~peios/security-descriptors/overview); each piece inherits one from the directory it is created in.

## Patterns

Each `pattern` argument marks a cut point. `csplit` copies everything up to that point into the next output file, then continues.

| Pattern | Cut where… |
|---|---|
| `N` (a number) | line `N` is reached — the piece is lines up to `N-1`. |
| `/REGEX/` | a line matches `REGEX`; that line starts the *next* piece. |
| `%REGEX%` | a line matches `REGEX` — but **skip** the text up to it instead of writing it to a piece. |
| `/REGEX/+N` or `/REGEX/-N` | an offset of `N` lines from the matching line. |
| `{N}` | repeat the *previous* pattern `N` more times. |
| `{*}` | repeat the previous pattern as many times as the file allows. |

A pattern that finds no match is an error, and by default `csplit` removes the pieces it had already written. `-k` keeps them.

## Naming the pieces

| Option | Effect |
|---|---|
| `-f`, `--prefix=PREFIX` | Use `PREFIX` instead of `xx` at the start of each output name. |
| `-n`, `--digits=N` | Use `N` digits in the numeric part of the name instead of 2. |
| `-b`, `--suffix-format=FORMAT` | Use a custom `printf`-style `FORMAT` for the numeric suffix. |

## Other options

| Option | Effect |
|---|---|
| `-k`, `--keep-files` | Do not delete the output files if `csplit` fails partway. |
| `-z`, `--elide-empty-files` | Do not write out pieces that would be empty. |
| `--suppress-matched` | Drop the lines that the patterns matched, rather than keeping them in a piece. |
| `-s`, `--quiet` | Do not print the byte size of each piece. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | The file was split. |
| `1` | A pattern did not match, a line number was out of range, or the input could not be read. |
