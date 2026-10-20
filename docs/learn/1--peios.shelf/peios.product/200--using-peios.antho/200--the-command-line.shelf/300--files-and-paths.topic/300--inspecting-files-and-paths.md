---
title: Inspecting Files and Paths
type: how-to
description: List directories, read file metadata, and resolve, split, or check paths with the Peios command-line utilities, choosing how much each check must resolve.
related:
  - peios/files-and-paths/files-names-and-security
  - peios/files-and-paths/managing-files
  - peios/permissions/sd
---

Use these commands to see what is in a directory, read a file's metadata, and
work out what a path refers to. A path can be text you are building, a link's
stored target, or a route to an existing file, and each needs a different
command. The man page for each command lists every option.

The examples use sample file names such as `report.txt`. Create those files
first, or substitute your own.

## List a directory

Show a directory's entries, with their metadata:

```sh
ls -l sample-directory
```

Names that begin with a dot are hidden. `-a` includes them, and `-A` includes
them without the `.` and `..` entries. `dir` and `vdir` use the same listing
engine with fixed layouts: columns for `dir`, one detailed row per entry for
`vdir`.

The ownership and permission fields in a long listing are kept for
compatibility, and don't describe who can use the file. Use `sd show` for
that. Don't parse `ls` output in a script: names can contain spaces and
newlines, and the listing is meant for people.

To color listings, `dircolors -b` prints a shell setting that maps kinds of
file to colors. Review the settings before adding them to your shell
configuration, and keep listings readable without color.

## Read a file's metadata

Print selected properties of a file:

```sh
stat -c '%n: %s bytes' report.txt
```

Like `ls`, `stat` prints mode, user, and group fields for compatibility, and
they don't describe access. A symbolic link has metadata of its own, so decide
whether you want the link or the file it points to. A size can change before
you read the file, so a program that limits how much it reads checks the file
it has opened.

## Resolve a path

Choose the command by the question you are asking:

- **What does this link store?** `readlink latest` prints the path stored in a
  symbolic link. The path can be relative, and its target might not exist.
- **Where is this existing file?** `realpath -e reports/draft.txt` follows
  every link and prints the absolute path. With `-e`, every part of the path
  must exist.
- **Where would this new file go?** `realpath reports/planned.txt` allows the
  last part to be missing, and `realpath -m` allows missing parent directories
  too. The result is a planned path, not proof that you can create the file
  there.
- **Where am I?** `pwd -P` prints the working directory with every symbolic
  link resolved. Your shell also has its own `pwd`.

A resolved path doesn't reserve a name or check access, and another program
can change the file before you open it. The quiet mode of `realpath` hides
error messages but not a failing exit status, so check the status before using
the output.

## Split and check path text

`basename` and `dirname` work on the text of a path without reading the
filesystem:

```sh
basename reports/draft.txt .txt
dirname reports/draft.txt
```

The first prints `draft`, the second `reports`. Neither checks that the file
exists. `pathchk reports/draft.txt` checks that a proposed path meets naming
limits, but success doesn't promise that creating the file will work.

## Commands on this page

This page covers these commands:

| Command | Use it to | Manual |
|---|---|---|
| `ls` | List directory entries | `ls(1)` |
| `dir`, `vdir` | List entries in a fixed layout | `dir(1)`, `vdir(1)` |
| `dircolors` | Configure listing colors | `dircolors(1)` |
| `stat` | Read file or filesystem metadata | `stat(1)` |
| `readlink` | Read a symbolic link's stored path | `readlink(1)` |
| `realpath` | Resolve a path to an absolute path | `realpath(1)` |
| `pwd` | Print the working directory | `pwd(1)` |
| `basename`, `dirname` | Split a path's text | `basename(1)`, `dirname(1)` |
| `pathchk` | Check a proposed path name | `pathchk(1)` |
