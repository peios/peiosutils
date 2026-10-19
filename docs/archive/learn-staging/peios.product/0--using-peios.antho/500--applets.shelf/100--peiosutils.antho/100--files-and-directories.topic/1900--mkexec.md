---
title: mkexec
type: reference
description: Mark a regular file executable, so that the kernel is willing to run it.
related:
  - peios/files-and-directories/sd
  - peios/files-and-directories/overview
  - peios/security-descriptors/overview
---

`mkexec` marks a regular file as a program, so that the kernel is willing to run it.

```
mkexec file...
```

```
$ mkexec ./report-tool
```

## Why the mark exists

On Peios, who may do what to a file is decided by the file's [security descriptor](~peios/security-descriptors/overview). POSIX mode bits carry none of that authority, which is why Peios ships no `chmod`.

One part of the mode survives, with a different meaning. Before any access decision is made, the kernel refuses outright to execute a file that carries no execute bit anywhere. That refusal happens below the access-control layer, and it is not waived for administrators: an unmarked file is unrunnable for everybody, including Local System.

So on Peios the execute bit is not a permission. It is an intrinsic property of the file — a statement that the file is a program rather than data — and `mkexec` is how you set it.

## What it does not do

Marking a file executable grants nobody the right to execute it. That right comes from the file's security descriptor, and [`sd`](~peios/files-and-directories/sd) is the tool that grants it.

The two are separate steps, and a program needs both: the mark says the file *is* a program, the descriptor says *who* may run it. A file that is marked but not permitted fails the access check; a file that is permitted but not marked fails before the check is reached.

## When a file arrives unmarked

A file gets the mark when something deliberately sets it. A file written by a program that simply created it, or moved onto the system by a transport that did not carry the mark across, arrives without one — and refuses to run, usually reporting only "permission denied". `mkexec` is the fix.

```
$ ./deploy
sh: ./deploy: Permission denied
$ mkexec ./deploy
$ ./deploy
```

## Details

`mkexec` writes the mark to all three execute slots. Which slot carries it has no meaning under the Peios security model — the mark is a single property, and any slot satisfies it — so no arbitrary choice is made between them. Every other mode bit is left alone.

Marking a file that is already marked succeeds and changes nothing, so a script may mark the same file twice.

Symbolic links are followed. Marking a link marks the file it points at, which is the file that would actually be executed.

Only regular files can be marked. Directories, device nodes, and sockets are rejected: the mark means "this is a program", and nothing else can be one.

You can see the mark in the second column of [`ls -l`](~peios/listing-and-paths/ls), which shows `x` for a marked file and `-` for an unmarked one.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every file was marked. |
| `1` | A file could not be marked — it does not exist, it is not a regular file, or its security descriptor does not permit the change. |
