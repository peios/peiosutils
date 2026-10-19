---
title: Viewing and joining text
type: reference
description: The commands that print files to the screen, show parts of them, and combine or split text by line and by column.
related:
  - peios/viewing-and-joining-text/cat
  - peios/transforming-text/overview
---

Much of the work at a command line is reading text — looking at a file, checking the start or end of one, pulling a column out, stitching two files together. This topic covers the commands for that: viewing files, showing parts of them, and joining or splitting text by line and by column.

This page names every command in the topic. The companion topic, [Transforming text](~peios/transforming-text/overview), covers the commands that *reshape* text — sorting, de-duplicating, substituting characters.

## The commands

**Viewing a whole file**

| Command | Purpose |
|---|---|
| [`cat`](~peios/viewing-and-joining-text/cat) | Print one or more files straight through — and join several into one. |
| [`tac`](~peios/viewing-and-joining-text/tac) | Print a file with its lines in reverse — last line first. |
| [`more`](~peios/viewing-and-joining-text/more) | Show a file one screen at a time, pausing between screens. |
| [`nl`](~peios/viewing-and-joining-text/nl) | Print a file with its lines numbered. |
| [`od`](~peios/viewing-and-joining-text/od) | Dump a file's raw bytes in octal, hex, decimal, or character form. |

**Viewing part of a file**

| Command | Purpose |
|---|---|
| [`head`](~peios/viewing-and-joining-text/head) | Print the first lines (or bytes) of a file. |
| [`tail`](~peios/viewing-and-joining-text/tail) | Print the last lines of a file — and, with `-f`, keep printing as the file grows. |

**Cutting and combining**

| Command | Purpose |
|---|---|
| [`cut`](~peios/viewing-and-joining-text/cut) | Pull selected columns — byte ranges or delimited fields — out of each line. |
| [`paste`](~peios/viewing-and-joining-text/paste) | Join files side by side, line for line. |
| [`join`](~peios/viewing-and-joining-text/join) | Join two files on a shared field, like a database join. |
| [`comm`](~peios/viewing-and-joining-text/comm) | Compare two sorted files and report which lines they share. |

**Splitting a file into files**

| Command | Purpose |
|---|---|
| [`split`](~peios/viewing-and-joining-text/split) | Break a file into equal-sized pieces. |
| [`csplit`](~peios/viewing-and-joining-text/csplit) | Break a file into pieces at lines that match a pattern. |

## A note on input

Almost every command here follows the same convention for where its text comes from. Name one or more files and it reads those; name none and it reads from standard input, so it can sit in the middle of a pipeline. Where a command accepts files, the name `-` stands for standard input, so you can mix files and piped input in one invocation.

## Where to start

To simply print a file, or join a few together, read [`cat`](~peios/viewing-and-joining-text/cat) — the most-used command in the topic.
