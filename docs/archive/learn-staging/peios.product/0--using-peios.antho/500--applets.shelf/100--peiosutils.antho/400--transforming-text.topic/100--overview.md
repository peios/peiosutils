---
title: Transforming text
type: reference
description: The commands that reshape text — reordering lines, removing duplicates, substituting characters, reflowing paragraphs, and counting.
related:
  - peios/transforming-text/sort
  - peios/viewing-and-joining-text/overview
---

This topic covers the commands that **reshape** text: putting lines in order, dropping duplicates, swapping characters, reflowing paragraphs to a width, counting what is there. Its companion, [Viewing and joining text](~peios/viewing-and-joining-text/overview), covers printing and combining files; this topic is about changing the text itself.

## The commands

**Ordering lines**

| Command | Purpose |
|---|---|
| [`sort`](~peios/transforming-text/sort) | Sort lines — alphabetically, numerically, and many other ways. |
| [`shuf`](~peios/transforming-text/shuf) | The opposite of sorting: put lines into a random order. |
| [`tsort`](~peios/transforming-text/tsort) | Topological sort — order items so that each comes after the things it depends on. |

**Filtering lines**

| Command | Purpose |
|---|---|
| [`uniq`](~peios/transforming-text/uniq) | Collapse or report adjacent repeated lines. |

**Substituting characters**

| Command | Purpose |
|---|---|
| [`tr`](~peios/transforming-text/tr) | Translate, squeeze, or delete individual characters. |

**Whitespace**

| Command | Purpose |
|---|---|
| [`expand`](~peios/transforming-text/expand) | Convert tabs into spaces. |
| [`unexpand`](~peios/transforming-text/unexpand) | Convert runs of spaces back into tabs. |

**Reflowing and paginating**

| Command | Purpose |
|---|---|
| [`fmt`](~peios/transforming-text/fmt) | Reflow paragraphs to a target width. |
| [`fold`](~peios/transforming-text/fold) | Hard-wrap long lines at a fixed width. |
| [`pr`](~peios/transforming-text/pr) | Paginate and columnate text for printing. |

**Indexing and counting**

| Command | Purpose |
|---|---|
| [`ptx`](~peios/transforming-text/ptx) | Produce a permuted index of the words in a file. |
| [`wc`](~peios/transforming-text/wc) | Count the lines, words, and bytes in a file. |

## Grouping duplicates with sort and uniq

A theme worth knowing before you start: [`uniq`](~peios/transforming-text/uniq) only collapses *adjacent* duplicate lines, so duplicates scattered through a file are not caught unless the file is sorted first. `sort` and `uniq` are almost always used together — and `sort -u` does both jobs in one step.

## Where to start

[`sort`](~peios/transforming-text/sort) is the workhorse of the topic and the one with the most depth — start there.
