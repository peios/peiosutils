---
title: split
type: reference
description: Break a file into equal-sized pieces, each written to its own output file.
related:
  - peios/viewing-and-joining-text/csplit
  - peios/viewing-and-joining-text/cat
  - peios/viewing-and-joining-text/overview
---

`split` breaks one file into several smaller files of roughly equal size.

```
split [options] [input [prefix]]
```

```
$ split -l 1000 big.log
$ ls
xaa  xab  xac  xad
```

By default `split` writes 1000 lines per piece, names the pieces `xaa`, `xab`, `xac`, … — a `prefix` of `x` followed by a two-letter suffix — and reads its input from a named file or from standard input.

To put the file back together, concatenate the pieces in order: [`cat`](~peios/viewing-and-joining-text/cat)` xaa xab xac … > original`.

Each piece is a newly created file, and a new file needs a [security descriptor](~peios/security-descriptors/overview); each piece inherits one from the directory it is created in.

## How big each piece is

Choose one of these to set the piece size:

| Option | Each piece holds… |
|---|---|
| `-l`, `--lines=N` | `N` lines. This is the default behaviour, with `N` = 1000. |
| `-b`, `--bytes=SIZE` | `SIZE` bytes. |
| `-C`, `--line-bytes=SIZE` | up to `SIZE` bytes, but only whole lines — never a line split across two pieces. |
| `-n`, `--number=CHUNKS` | the input divided into a fixed number of pieces (see below). |

A `SIZE` accepts a unit suffix: `K`, `M`, `G`, … as powers of 1024, or `KB`, `MB`, … as powers of 1000.

### Fixed number of chunks (`-n`)

`-n` divides the input into a set number of pieces rather than fixing each piece's size. `CHUNKS` may be:

| Form | Effect |
|---|---|
| `N` | Split into `N` pieces by size. |
| `l/N` | Split into `N` pieces without splitting any line across pieces. |
| `r/N` | Like `l/N`, but distribute lines round-robin across the pieces. |
| `K/N` | Write only the `K`th of `N` pieces, to standard output. |

## Naming the pieces

| Option | Effect |
|---|---|
| `prefix` (operand) | The leading part of each output name. Default `x`. |
| `-a`, `--suffix-length=N` | Use `N` characters for the suffix. Default 2. |
| `-d`, `--numeric-suffixes[=START]` | Use numeric suffixes (`00`, `01`, …) instead of letters, optionally from `START`. |
| `-x`, `--hex-suffixes[=START]` | Use hexadecimal suffixes. |
| `--additional-suffix=SUFFIX` | Append a fixed `SUFFIX` to every output name — for giving the pieces an extension. |

Each piece is created as a new file. If a name already exists, `split` opens it and checks, on the open handle, that it is not the input before truncating it; an output that turns out to be the input is refused with "would overwrite input".

## Other options

| Option | Effect |
|---|---|
| `-e`, `--elide-empty-files` | With `-n`, do not write out pieces that would be empty. |
| `-t`, `--separator=SEP` | Use `SEP` as the line separator instead of newline. |
| `--verbose` | Print a line as each output file is opened. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | The file was split. |
| `1` | A failure — the input could not be read, or the suffixes were exhausted. |
