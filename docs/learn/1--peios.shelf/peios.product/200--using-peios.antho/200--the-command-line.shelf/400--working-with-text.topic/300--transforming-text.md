---
title: Transforming Text
type: how-to
description: Sort and count records, translate characters, convert between tabs and spaces, reflow and paginate text, order dependencies, and shuffle lists.
related:
  - peios/working-with-text/text-streams
  - peios/working-with-text/viewing-and-combining-text
---

Use these commands to change the order, characters, or layout of text. Each
writes a transformed copy to standard output and leaves its input unchanged.
Decide which properties of the text carry meaning before you transform it: a
reflow can break code, and a sort loses the original order. The man page for
each command lists every option.

The examples use sample file names such as `names.txt`. Create those files
first, or substitute your own.

## Sort and count records

Sort lines as numbers rather than as text:

```sh
LC_ALL=C sort -n counts.txt
```

Without `-n`, `sort` compares text, so `12` sorts before `3`. Use a key field
when each line holds both a name and a value. To write the result over the
input, use the command's output option rather than shell redirection, which
would empty the input first. See `sort(1)`.

`uniq` compares neighboring lines only, so sort first to count every repeated
value:

```sh
LC_ALL=C sort names.txt | uniq -c
```

`wc -l -w -c names.txt` counts lines, words, and bytes. The line count counts
newline characters, so a last line with no newline isn't counted. Byte and
character counts differ for characters that take more than one byte.

## Translate characters

`tr` changes or removes characters in standard input:

```sh
printf 'aster birch\n' | LC_ALL=C tr 'a-z' 'A-Z'
```

`tr` reads only standard input; it takes no file name. A range such as `a-z`
doesn't convert letters in every writing system, so keep to ASCII text or use
a tool that understands the language. Quote the character sets so that the
shell doesn't interpret them.

## Convert tabs and spaces

`expand -t 4 tabs.txt` replaces each tab with the spaces needed to reach the
next tab stop, and `unexpand -t 4 spaces.txt` turns suitable runs of spaces
back into tabs. Without `-t`, `expand` puts tab stops eight columns apart.
Keep the original when tabs carry meaning in the file's format.

## Reflow and paginate

`fmt -w 30 notes.txt` rejoins the words of each paragraph into lines of at
most 30 characters. Use it for prose only: it changes the line breaks that
code, tables, and markup depend on. `fold -s -w 12` breaks long lines without
joining short ones. `pr` adds page headings and margins for printing, so don't
pass its output to a program that expects the original lines.

## Order dependencies

`tsort` reads pairs in which the first item must come before the second, and
prints an order that satisfies every pair:

```sh
printf 'prepare build\nbuild test\n' | tsort
```

If the pairs form a cycle, no order exists and `tsort` fails; check its exit
status rather than using partial output. Items with no dependency between them
can come out in any order.

## Shuffle and index

`shuf names.txt` prints the lines in a random order, and can take a sample.
Given a recorded random source, it repeats the same shuffle, which helps in
testing but isn't suitable for choices that must be unpredictable. `ptx`
builds a keyword index that shows each word with the text around it.

## Commands on this page

This page covers these commands:

| Command | Use it to | Manual |
|---|---|---|
| `sort` | Sort lines | `sort(1)` |
| `uniq` | Collapse or count repeated neighboring lines | `uniq(1)` |
| `wc` | Count lines, words, and bytes | `wc(1)` |
| `tr` | Translate or delete characters | `tr(1)` |
| `expand`, `unexpand` | Convert between tabs and spaces | `expand(1)`, `unexpand(1)` |
| `fmt` | Reflow paragraphs | `fmt(1)` |
| `fold` | Wrap long lines | `fold(1)` |
| `pr` | Paginate text for printing | `pr(1)` |
| `tsort` | Order items by their dependencies | `tsort(1)` |
| `shuf` | Shuffle or sample lines | `shuf(1)` |
| `ptx` | Build a keyword-in-context index | `ptx(1)` |
