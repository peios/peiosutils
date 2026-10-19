---
title: Ordering and Transforming Records
type: concept
description: >-
  Choose comparison rules and record boundaries before transforming text.
related:
  - peios/transforming-text/sort
  - peios/viewing-and-joining-text/overview
---

Text transformations alter order, spacing, or content. Decide which properties
carry meaning before choosing a command that changes them.

## Ordering changes neighboring relationships

`uniq` sees adjacent duplicates. Sorting first can collect equal values, but
it also removes the original ordering information. A join or comparison needs
inputs sorted under compatible rules.

Locale, numeric interpretation, and field selection can all affect comparison.
Keep those settings consistent across a pipeline.

## Choose the transformation

Use these guides to preserve the properties your task needs:

- [Sorting records](~peios/transforming-text/sort) selects comparison rules.
- [Counting adjacent repeats](~peios/transforming-text/uniq) explains grouping.
- [Translating characters](~peios/transforming-text/tr) changes character sets.
- [Reflowing paragraphs](~peios/transforming-text/fmt) changes prose layout.
- [Wrapping physical lines](~peios/transforming-text/fold) retains line separation.
- [Ordering dependencies](~peios/transforming-text/tsort) respects prerequisite pairs.
- [Counting text](~peios/transforming-text/wc) distinguishes lines and bytes.

## Validate the transformed result

A valid text stream can still be the wrong format for its consumer. Reflowing
code, splitting a multibyte character, or counting an unterminated record as a
newline can each change the intended meaning.
