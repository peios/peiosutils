---
title: Files, Names, and Security
type: concept
description: >-
  Choose creation, copying, linking, and removal operations by their effect
  on file identity.
related:
  - peios/files-and-directories/cp
  - peios/files-and-directories/mkdir
---

File commands change names, contents, or metadata. Distinguishing those
changes helps you choose an operation without losing data or assuming the
wrong security behavior.

## A name and an object are different

A file can have several hard-link names. Renaming a name can preserve the same
object, while copying creates a different object. Symbolic links store paths
that can later resolve to different objects.

That distinction affects security. A renamed object keeps its descriptor; a
newly created object normally inherits from its destination. Inheritance is
not a live link that continuously refreshes existing children.

## Choose the effect you need

Use these guides to choose the operation:

- [Copying files](~peios/files-and-directories/cp) creates independent data.
- [Moving files](~peios/files-and-directories/mv) changes where files are named.
- [Creating links](~peios/files-and-directories/ln) adds a name or indirection.
- [Creating directories](~peios/files-and-directories/mkdir) prepares a tree.
- [Inspecting security](~peios/permissions/sd) separates access components.
- [Removing files](~peios/files-and-directories/rm) removes names deliberately.

## Check status and resulting state

A recursive operation can fail after some changes succeeded. A successful
status can also include intentional skips under an update policy. Inspect the
resulting state when completeness matters.

Deletion is not a universal data-erasure method. Filesystem snapshots,
backups, and device behavior can retain copies beyond the visible name.
