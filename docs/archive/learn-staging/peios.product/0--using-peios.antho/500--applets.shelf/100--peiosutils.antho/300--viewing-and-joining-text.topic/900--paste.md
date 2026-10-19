---
title: paste
type: reference
description: Join files side by side, line for line.
related:
  - peios/viewing-and-joining-text/cut
  - peios/viewing-and-joining-text/join
  - peios/viewing-and-joining-text/overview
---

`paste` joins files **side by side**. It takes the first line of each file and prints them on one line, then the second line of each, and so on — merging files into columns.

```
paste [options] [file...]
```

```
$ paste names.txt ages.txt
Ada      36
Grace    41
Linus    29
```

By default the merged pieces are separated by a tab. `paste` is the column-wise counterpart of [`cat`](~peios/viewing-and-joining-text/cat), which joins files end to end.

## Pasting one file at a time

| Option | Effect |
|---|---|
| `-s`, `--serial` | Paste each file's lines onto a *single line*, one file at a time, instead of merging files in parallel. A file's lines become a row. |

```
$ paste -s -d , names.txt
Ada,Grace,Linus
```

## Choosing the separator

| Option | Effect |
|---|---|
| `-d`, `--delimiters=LIST` | Use the characters in `LIST` as separators instead of a tab. When `LIST` has more than one character, `paste` cycles through them. |
| `-z`, `--zero-terminated` | Treat the NUL character as the line delimiter instead of newline. |

## `paste` and `join`

`paste` merges by *position* — line 1 with line 1, line 2 with line 2, blind to content. When you want to merge by a *matching value* — pairing rows that share a key — that is [`join`](~peios/viewing-and-joining-text/join).

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A file could not be read, or the delimiter list was malformed. |
