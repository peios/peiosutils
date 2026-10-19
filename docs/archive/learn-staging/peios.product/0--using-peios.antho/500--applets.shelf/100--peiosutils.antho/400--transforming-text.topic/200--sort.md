---
title: sort
type: reference
description: Sort the lines of files — alphabetically, numerically, by field, and many other ways.
related:
  - peios/transforming-text/uniq
  - peios/transforming-text/shuf
  - peios/transforming-text/overview
---

`sort` reads lines and writes them out in order.

```
sort [options] [file...]
```

```
$ sort names.txt
```

With no file, `sort` reads standard input. Given several files, it sorts all of their lines together as one stream. By default it compares lines as plain text, character by character.

## How lines are compared

The default text comparison is often not the order you want — `10` sorts before `9`, and case matters. These options change the comparison:

| Option | Compares lines as… |
|---|---|
| `-n`, `--numeric-sort` | numbers. `9` sorts before `10`. |
| `-g`, `--general-numeric-sort` | numbers, including scientific notation. Slower than `-n`; use it only when `-n` is not enough. |
| `-h`, `--human-numeric-sort` | human-readable sizes — `2K` before `1M` before `3G`. |
| `-M`, `--month-sort` | month names — `JAN` before `FEB`. |
| `-V`, `--version-sort` | version numbers — `1.2.10` after `1.2.9`. |
| `-R`, `--random-sort` | a random order (a shuffle that groups equal lines together). |

And these adjust *what* is compared:

| Option | Effect |
|---|---|
| `-f`, `--ignore-case` | Treat lower and upper case as the same. |
| `-d`, `--dictionary-order` | Consider only letters, digits, and blanks. |
| `-i`, `--ignore-nonprinting` | Ignore non-printing characters. |
| `-b`, `--ignore-leading-blanks` | Ignore blanks at the start of a line (or field). |

## Sort order options

| Option | Effect |
|---|---|
| `-r`, `--reverse` | Reverse the result. |
| `-u`, `--unique` | Output only the first line of each run of equal lines — sort and de-duplicate in one step. |
| `-s`, `--stable` | Keep equal lines in their original relative order. |

## Sorting by a field

By default `sort` compares whole lines. `-k` sorts on a **key** — a chosen part of each line — which is what you need for tabular data.

A line is split into fields (by default at whitespace). A key is written `FIELD[.CHAR][OPTIONS][,FIELD[.CHAR]]` — a start field, an optional start character within it, and an optional end. The single-letter comparison options above can be appended to a key to apply only to it.

```
$ sort -k 2 -n scores.txt        # sort on the 2nd field, numerically
$ sort -t , -k 3 people.csv      # sort a CSV on the 3rd field
```

| Option | Effect |
|---|---|
| `-k`, `--key=KEYDEF` | Sort on the key `KEYDEF`. May be given more than once; keys are tried in order. |
| `-t`, `--field-separator=SEP` | Use `SEP` to split fields, instead of the whitespace default. |

## Checking and merging

| Option | Effect |
|---|---|
| `-c`, `--check` | Do not sort — just check whether the input is already sorted, and report the first line that is out of order. |
| `-C`, `--check=silent` | Check, but report only through the exit status. |
| `-m`, `--merge` | Treat the inputs as already-sorted files and merge them, which is faster than a full sort. |

## Output and resources

| Option | Effect |
|---|---|
| `-o`, `--output=FILE` | Write to `FILE` instead of standard output. `FILE` may be one of the inputs. |
| `-z`, `--zero-terminated` | Treat the NUL character as the line delimiter. |
| `--parallel=N` | Use `N` threads. |
| `-S`, `--buffer-size=SIZE` | Set the size of the in-memory sort buffer. |
| `-T`, `--temporary-directory=DIR` | Put temporary files in `DIR` rather than the default location. |
| `--files0-from=F` | Take the list of input files from `F`, NUL-separated. |
| `--debug` | Underline the part of each line that was actually used as the sort key — invaluable when a `-k` key is not behaving. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | The lines were sorted — or, with `-c`/`-C`, the input was already sorted. |
| `1` | With `-c`/`-C`, the input was *not* sorted. |
| `2` | An error — a file could not be read, or an option was invalid. |
