---
title: Applying a Root Skeleton
type: concept
description: >-
  Understand how a new root gets the directories and links outside /usr that
  no package can ship.
related:
  - peios/boot/mkirf
---

`skeleton apply` makes the directories and links a root needs outside `/usr`.
Packages only install under `/usr`, so `/var`, `/lcl`, `/tmp` and the `/lib64`
link to the dynamic loader come from a root skeleton file instead. The
`dev.peios.fsbase` package ships that file at `/usr/share/skeleton`, and
whatever builds a root applies the copy in that root's own `/usr`.

```sh
skeleton apply /mnt/root
```

The live-boot hook runs this after composing the live root. An installer or an
upgrade tool runs it against the root it builds. `--file` applies another file
instead of the root's own, and `--verbose` prints each entry it makes.

## Making what is missing

`skeleton` reads and checks the whole file, including every security
descriptor, before it changes anything. A file with an error changes nothing.

It then works through the entries in order. It makes each missing directory or
link, and leaves each existing one alone, even if its security descriptor
differs from the file's. Running it again makes nothing new.

It stops with an error when something of another kind is in an entry's place,
such as a file where a directory belongs, or a link with a different target.
It never follows a link to reach an entry. Entries made before the error stay,
so you can run it again after removing what was in the way.

## Descriptors at creation

A directory with a descriptor in the file is created with that descriptor in
the same native create call. It never exists with an inherited descriptor
first. A directory without one inherits from its parent.

The create fails if the new descriptor grants its creator no right to read the
directory. The skeleton's descriptors grant SYSTEM full control, and root
builders run as SYSTEM.

The file's format is specified in PSPU 11.
