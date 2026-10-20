---
title: Viewing and Combining Text
type: how-to
description: Read whole files or their start and end, follow a growing log, select fields, combine files by position or by key, and split a file into pieces.
related:
  - peios/working-with-text/text-streams
  - peios/working-with-text/transforming-text
---

Use these commands to read text files, pick out the parts you need, and
combine or split them. None of them change the files they read; they write
their result to standard output. The man page for each command lists every
option.

The examples use sample file names such as `names.txt`. Create those files
first, or substitute your own.

## Read a file

`cat` writes files to standard output one after another, which also combines
them:

```sh
cat opening.txt details.txt > report.txt
```

`cat -A` shows tabs, line endings, and other invisible characters, and
`cat -n` numbers the lines. For a long file, `more` shows one screen at a
time: press <kbd>Space</kbd> for the next screen, <kbd>/</kbd> to search, and
<kbd>Q</kbd> to stop. `nl` prints a numbered copy that leaves blank lines
unnumbered, and `tac` prints lines from last to first.

## Read the start or end

Show the first two lines of a file:

```sh
head -n 2 names.txt
```

`tail -n 2 progress.log` shows the last two lines. Without `-n`, `head` shows
10 lines. To watch a log as it grows, use `tail -f`, which follows the file it
opened. If a program replaces the log with a new file of the same name,
`tail -F` follows the name instead and keeps going. Press
<kbd>Ctrl</kbd>+<kbd>C</kbd> to stop. With several files, `head` and `tail`
label each one, and the labels become part of the output.

## Inspect raw bytes

When a terminal would hide or reinterpret characters, `od` shows each byte as
a number:

```sh
od -An -tx1 sample.bin
```

`-tx1` shows one byte per hexadecimal value, and `-An` hides the offset
column. Repeated rows are collapsed to `*` unless you add `-v`.

## Select fields

`cut` selects fields from each line, split at a delimiter:

```sh
cut -d : -f 1 counts.txt
```

A line without the delimiter passes through unchanged unless you add `-s`.

## Combine files

Choose by what links the lines together:

- **By position.** `paste names.txt counts.txt` puts the first line of each
  file side by side, then the second, and so on. A missing line shifts every
  later pair.
- **By a shared key.** `join counts.txt colors.txt` combines lines whose first
  field matches. Both files must be sorted on that field with the same rules,
  and a repeated key produces more than one output line.
- **By membership.** `comm` compares two sorted files and shows the lines
  found only in the first, only in the second, or in both.

For `join` and `comm`, set the locale to the one you sorted with, for example
`LC_ALL=C join counts.txt colors.txt`.

## Split a file

Split a file into pieces of two lines each:

```sh
split -l 2 original.txt piece-
```

The pieces are named `piece-aa`, `piece-ab`, and so on, and joining them in
that order with `cat` restores the original. `csplit report.txt '/Section/'`
splits at a line that matches a pattern instead. Both commands can overwrite
existing files with the same names, so run them in an empty directory. New
pieces inherit security from their directory.

## Commands on this page

This page covers these commands:

| Command | Use it to | Manual |
|---|---|---|
| `cat` | Write files to output in order | `cat(1)` |
| `more` | Read a file one screen at a time | `more(1)` |
| `nl` | Number lines | `nl(1)` |
| `tac` | Print lines in reverse order | `tac(1)` |
| `head`, `tail` | Read the start or end of a file | `head(1)`, `tail(1)` |
| `od` | Show bytes as numbers | `od(1)` |
| `cut` | Select fields from each line | `cut(1)` |
| `paste` | Combine lines by position | `paste(1)` |
| `join` | Combine lines by a shared key | `join(1)` |
| `comm` | Compare two sorted files | `comm(1)` |
| `split`, `csplit` | Split a file into pieces | `split(1)`, `csplit(1)` |
