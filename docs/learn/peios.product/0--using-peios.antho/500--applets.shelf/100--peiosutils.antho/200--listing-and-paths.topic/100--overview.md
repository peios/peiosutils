---
title: Names, Paths, and File Views
type: concept
description: >-
  Distinguish listing names, reading metadata, and resolving filesystem
  paths.
related:
  - peios/listing-and-paths/ls
  - peios/listing-and-paths/stat
---

Listing and path commands answer different questions about a filesystem.
Choose whether you need visible names, metadata for an object, or a spelling
that resolves through directories and links.

## A display is not a filename protocol

`ls` makes entries readable to a person. Names can contain whitespace or
control characters, so a human listing is not a reliable machine-readable list
of paths. Keep original names separate from display labels in tools.

`stat` exposes selected metadata. A query observes one moment, and its result
can change before another operation opens the file.

## Choose how much to resolve

These guides separate common tasks:

- [Inspecting a directory](~peios/listing-and-paths/ls) shows its entries.
- [Inspecting metadata](~peios/listing-and-paths/stat) selects file properties.
- [Choosing path resolution](~peios/listing-and-paths/choose-how-to-resolve-a-path) sets existence requirements.
- [Reading a symbolic link](~peios/listing-and-paths/readlink) returns stored target text.
- [Extracting a filename](~peios/listing-and-paths/basename) operates on path text.
- [Checking a proposed name](~peios/listing-and-paths/pathchk) checks naming constraints.

A normalized path is not a reservation, an authorization check, or a
persistent handle. Use the opened object's identity when concurrent changes
matter.
