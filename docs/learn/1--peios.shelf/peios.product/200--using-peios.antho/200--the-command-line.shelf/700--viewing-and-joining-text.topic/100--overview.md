---
title: Text Streams and Record Boundaries
type: concept
description: >-
  Choose commands by whether they operate on bytes, lines, positions, or
  matching keys.
related:
  - peios/viewing-and-joining-text/cat
  - peios/transforming-text/overview
---

Text utilities read streams of bytes and apply a chosen record model. Most use
newlines to divide records, but a delimiter does not turn arbitrary input into
a fully parsed document format.

## Match the operation to the relationship

Use these guides for common relationships:

- [Combining whole files](~peios/viewing-and-joining-text/cat) retains file order.
- [Reading the beginning](~peios/viewing-and-joining-text/head) previews an input.
- [Reading the end](~peios/viewing-and-joining-text/tail) inspects recent records.
- [Selecting fields](~peios/viewing-and-joining-text/cut) extracts simple columns.
- [Combining corresponding lines](~peios/viewing-and-joining-text/paste) uses position.
- [Matching shared keys](~peios/viewing-and-joining-text/join) uses field identity.
- [Inspecting raw bytes](~peios/viewing-and-joining-text/od) avoids text rendering.

## Keep source and output distinct

Shell redirection opens the output before the command starts reading. Using
the same file for both can destroy the input. Write a separate result and
check it before replacing an accepted source.

A command's output can be partial when it exits with an error. Check the
status of the operation that produced the data, not merely whether an output
file exists.
