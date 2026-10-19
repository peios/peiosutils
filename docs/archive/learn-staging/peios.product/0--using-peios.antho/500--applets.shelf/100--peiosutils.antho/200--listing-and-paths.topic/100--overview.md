---
title: Listing and paths
type: reference
description: The commands that show you what a directory contains and that inspect, resolve, and manipulate path names — ls, stat, pwd, readlink, realpath, and the rest.
related:
  - peios/listing-and-paths/ls
  - peios/listing-and-paths/stat
  - peios/security-descriptors/overview
---

Before you can do anything with a file you usually have to *find* it — see what a directory holds, learn one file's details, turn a relative name into an absolute one. This topic covers the commands for that: listing directories, inspecting a single file, and resolving path names.

This page names every command in the topic, says in one line what each is for, and points you at the page that covers it in full.

For a worked example of choosing what must exist, start with [Choose how to resolve a path](~peios/listing-and-paths/choose-how-to-resolve-a-path).

## The commands

| Command | Purpose |
|---|---|
| [`ls`](~peios/listing-and-paths/ls) | List the contents of a directory. The everyday command for "what's in here?". |
| [`dir` and `vdir`](~peios/listing-and-paths/dir-and-vdir) | `ls` with a fixed output style. `dir` lists in columns; `vdir` lists in long format. |
| [`stat`](~peios/listing-and-paths/stat) | Show the detailed status of one file — its type, size, timestamps, and raw inode metadata. |
| [`pwd`](~peios/listing-and-paths/pwd) | Print the directory you are currently working in. |
| [`readlink`](~peios/listing-and-paths/readlink) | Print where a symbolic link points. |
| [`realpath`](~peios/listing-and-paths/realpath) | Resolve a path to its canonical, absolute form — every symlink followed, every `.` and `..` removed. |
| [`basename`](~peios/listing-and-paths/basename) | Strip the directory part off a path, leaving just the final name. |
| [`dirname`](~peios/listing-and-paths/dirname) | The opposite of `basename` — strip the final name, leaving the directory. |
| [`pathchk`](~peios/listing-and-paths/pathchk) | Check whether a path name is valid and portable before you try to create it. |
| [`dircolors`](~peios/listing-and-paths/dircolors) | Produce the colour settings that `ls` uses when it colours its output. |

## These commands and file security

`basename` and `dirname` split path strings without consulting the file system. `realpath` normally resolves links through the file system, with options for missing components or text-only handling. `pathchk` checks names; its default checks include file-system length limits and whether leading directories can be searched.

`ls` and `stat` report on real files, and they relate to file security differently:

- **`ls -l` surfaces security-descriptor information directly.** On Peios a file's access is governed by its **security descriptor** — the record of who owns the file and who may do what to it. The long listing shows the owner and the parts of the descriptor that fit in a one-line-per-file format.
- **`stat` is a low-level diagnostic.** It dumps the raw contents of a file's inode. The inode carries some numeric fields — an owner id, a group id, a mode value — that the Peios access model does **not** consult. `stat` reports them because it reports the inode verbatim, but they are not the file's access policy.

Neither command prints the full security descriptor; inspecting that is a job for the dedicated security tooling. If a line of `ls -l` looks unfamiliar, or you want to know why `stat` shows fields that "don't count", the explanation is in [Security descriptors](~peios/security-descriptors/overview).

## Where to start

If you want the everyday "what's in this directory" command and the meaning of every column it prints, read [`ls`](~peios/listing-and-paths/ls).

If you need the full detail of one specific file, read [`stat`](~peios/listing-and-paths/stat).

If you are writing a script, use [`basename`](~peios/listing-and-paths/basename) or [`dirname`](~peios/listing-and-paths/dirname) to split a path as text. Use [`realpath`](~peios/listing-and-paths/realpath) to resolve it with the existence and link-handling options your task needs.
