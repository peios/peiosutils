---
title: Files and directories
type: reference
description: The commands that create, copy, move, remove, and resize files and directories — and how each one handles a file's security descriptor.
related:
  - peios/files-and-directories/cp
  - peios/files-and-directories/mkdir
  - peios/security-descriptors/overview
---

This topic covers commands for creating, copying, moving, removing, and resizing files and directories, as well as reporting disk usage. Commands in other topics can also write output files.

This page names every command, and then explains the one theme that runs through the whole topic on Peios: what happens to a file's **security descriptor** when the file is created or copied.

## The commands

| Command | Purpose |
|---|---|
| [`cp`](~peios/files-and-directories/cp) | Copy files and directories. |
| [`mv`](~peios/files-and-directories/mv) | Move or rename files and directories. |
| [`rm`](~peios/files-and-directories/rm) | Remove files, and with `-r`, directories. |
| [`rmdir`](~peios/files-and-directories/rmdir) | Remove directories, but only empty ones. |
| [`mkdir`](~peios/files-and-directories/mkdir) | Create directories. |
| [`mkfifo`](~peios/files-and-directories/mkfifo) | Create named pipes (FIFOs). |
| [`mknod`](~peios/files-and-directories/mknod) | Create special files — device nodes and FIFOs. |
| [`touch`](~peios/files-and-directories/touch) | Create empty files, or update a file's timestamps. |
| [`ln`](~peios/files-and-directories/ln) | Create hard links and symbolic links. |
| [`link` and `unlink`](~peios/files-and-directories/link-and-unlink) | The single-purpose, low-level versions of "make a hard link" and "remove a file". |
| [`shred`](~peios/files-and-directories/shred) | Destroy a file's contents by overwriting them, so they cannot be recovered. |
| [`dd`](~peios/files-and-directories/dd) | Copy data block by block, converting it on the way. |
| [`df`](~peios/files-and-directories/df) | Report how much space each file system has, and how much is free. |
| [`du`](~peios/files-and-directories/du) | Report how much space files and directories use. |
| [`truncate`](~peios/files-and-directories/truncate) | Shrink or extend a file to an exact size. |
| [`mktemp`](~peios/files-and-directories/mktemp) | Create a temporary file or directory with a safely unique name. |
| [`sd`](~peios/files-and-directories/sd) | Inspect selected file security components and change file security descriptors. |
| [`mkexec`](~peios/files-and-directories/mkexec) | Mark a regular file as a program, so the kernel is willing to run it. |

## Every file has a security descriptor

On Peios, every file and directory carries a **security descriptor** — the record of who owns it and who may do what to it. Access to a file is decided from its descriptor; see [Security descriptors](~peios/security-descriptors/overview) for the full picture.

That fact shapes two groups of commands in this topic.

### Creating a file gives it a descriptor

When `mkdir`, `mkfifo`, `mknod`, or `touch` creates a new object, that object needs a security descriptor. By default it gets one **inherited from the directory it is created in** — the new object picks up the access rules its parent directory hands down.

All four of these commands share one set of options — the **creation flags** — that let you set the new object's descriptor explicitly instead of taking the inherited default: name an owner, set a different group, lock the object so it does *not* inherit, give it an integrity label, or supply a complete descriptor. The flags are documented in full on the [`mkdir`](~peios/files-and-directories/mkdir) page; `mkfifo`, `mknod`, and `touch` each link back to it.

### Copying a file makes a fresh descriptor — unless you preserve

`cp` creates new files, and a cross-file-system `mv` does too. A brand-new copy gets a brand-new descriptor inherited from its destination directory — it does **not** carry the source file's security across by default.

When you *do* want the copy to keep the source's owner, access rules, timestamps, or other attributes, that is the job of the `--preserve` family of options. `cp` and `mv` share an identical `--preserve` surface; it is documented on the [`cp`](~peios/files-and-directories/cp) page.

### Removing a file checks whether you may write it

`rm` and `shred` both make a decision based on whether you can write a file. `rm` prompts before removing a file you cannot write to; `shred`'s `--force` overrides a file's protection so it can be destroyed. Both judge "can you write this?" by a live access check against the file's security descriptor — not by any decorative metadata on the inode. Each command's page explains exactly where that check is made.

## Where to start

For copying — and for the whole `--preserve` model — read [`cp`](~peios/files-and-directories/cp).

For creating files and directories, and the shared creation flags, read [`mkdir`](~peios/files-and-directories/mkdir).

For removing things safely, read [`rm`](~peios/files-and-directories/rm).
