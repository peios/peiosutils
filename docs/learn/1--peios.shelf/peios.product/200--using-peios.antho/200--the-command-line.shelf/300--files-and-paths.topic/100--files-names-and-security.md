---
title: Files, Names, and Security
type: concept
description: The difference between a file's name and the object it names, which security a copied, moved, or new file receives, and why listings and path checks describe only a moment.
related:
  - peios/files-and-paths/managing-files
  - peios/files-and-paths/inspecting-files-and-paths
  - peios/permissions/sd
---

File commands change names, contents, or metadata. Knowing which of those a
command changes tells you whether data can be lost, and which security the
result has. Every file on Peios has a security descriptor that decides who can
use it, and copying, moving, and creating files treat that descriptor
differently.

## A name and an object are different

A file is an object with contents, metadata, and a security descriptor. A name
is an entry in a directory that refers to the object. One object can have
several names, called hard links, and removing one name leaves the others. A
symbolic link is a separate kind of file that stores a path, and that path can
later lead to a different object, or to nothing.

## Which security a file gets

The command you use decides which security descriptor the result has:

| Operation | Result |
|---|---|
| Rename a file, including moving it within one filesystem | The same object, with its own security descriptor |
| Copy a file, including moving it to another filesystem | A new object, which inherits security from its destination directory |
| Create a file or directory | A new object, which inherits security from its parent directory |
| Add a hard link | Another name for the same object and descriptor |

Moving a file into another directory doesn't apply that directory's inherited
rules to it. Inheritance happens once, when an object is created, and doesn't
keep existing objects in step with their parent.

`cp`, `mv`, `mkdir`, and the other creation commands can request specific
security instead. [Managing Files and
Directories](~peios/files-and-paths/managing-files) shows how. To see or
change a file's security descriptor, use [`sd`](~peios/permissions/sd).

## Results that need checking

A recursive operation can fail after it has already changed part of a tree,
and a command can report success while skipping files under an update policy.
Check the result when every file matters.

Removing a name doesn't erase its data. Other hard links, snapshots, backups,
and the storage device itself can keep copies.

## What a listing shows

A listing is for people to read. File names can contain spaces, tabs, and
newlines, so don't parse `ls` output to find file names in a script.

The mode, user, and group fields that `ls` and `stat` print are kept for
compatibility. They summarize some metadata, but they don't describe who can
use a file: the security descriptor does.

Every listing, metadata query, or path check describes one moment. The file
can change, be replaced, or disappear before the next command runs, so a
program that must act on a particular file checks the file it opens.
