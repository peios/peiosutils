---
title: Text Streams
type: concept
description: How the text utilities divide input into records, why output must go to a different file than the input, and why sorting rules have to match across a pipeline.
related:
  - peios/working-with-text/viewing-and-combining-text
  - peios/working-with-text/transforming-text
  - peios/working-with-text/checksums-and-encodings
---

The text utilities read a stream of bytes and divide it into records, usually
one record for each line. Each command then does one thing to those records:
shows some of them, combines them, reorders them, or changes their characters.
You can join the commands in a pipeline, where each one reads the previous
command's output. A few rules decide whether the result means what you expect.

## Records and delimiters

Most commands end a record at each newline. Commands that select fields, such
as `cut` and `join`, also split each record at a delimiter character you
choose. A delimiter is only a character, not a parsing rule: these commands
don't understand quoted values or escaped delimiters, so use a parser for
formats that have them.

Commands that count bytes rather than lines or characters can split a
character that takes more than one byte. Use line or character counts for text
that people will read.

## Keep input and output separate

The shell opens an output file before the command starts. Redirecting output
to the file a command is reading empties that file before the command reads
it:

```sh
sort counts.txt > sorted-counts.txt
```

Always write to a different file, check it, and only then replace the
original. A command that exits with an error can leave partial output, so
check its exit status, not only whether the output file exists.

## Order and comparison rules

Some commands compare records with each other. `uniq` compares neighboring
lines only, so sort first to bring equal lines together. `join` and `comm`
need both inputs sorted with the same rules.

Sorting depends on the locale, which sets the language rules for comparing
text. Set the same locale for every command in a pipeline that compares
records. `LC_ALL=C` gives an ordering by byte value that is the same on every
machine:

```sh
LC_ALL=C sort names.txt
```

## Commands for each task

Choose the page by what you want to do:

- [Viewing and Combining
  Text](~peios/working-with-text/viewing-and-combining-text) to read files,
  select parts of them, and combine or split them.
- [Transforming Text](~peios/working-with-text/transforming-text) to sort,
  count, translate, and reformat text.
- [Verifying Checksums and Encoding
  Bytes](~peios/working-with-text/checksums-and-encodings) to check that data
  hasn't changed, or to carry bytes as text.
