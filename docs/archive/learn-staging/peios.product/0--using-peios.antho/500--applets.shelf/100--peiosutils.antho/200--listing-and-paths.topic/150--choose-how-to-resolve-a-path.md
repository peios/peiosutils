---
title: Choose how to resolve a path
type: how-to
description: Choose between reading a link, resolving existing input, and preparing a path for a file that does not exist yet.
related:
  - peios/listing-and-paths/overview
  - peios/listing-and-paths/realpath
  - peios/listing-and-paths/readlink
---

When a script receives a path, first decide what must exist. Resolving an existing input, naming a new output file, and planning a directory tree need different checks.

## Start with one working directory

Assume the physical working directory is `/home/jack/project`, with no symbolic links in that directory path:

- `notes.txt` is an existing ordinary file.
- `latest` is a symbolic link whose stored target is `notes.txt`.
- `report.txt` does not exist yet.
- `planned` and its child directory `drafts` do not exist yet.

The examples use those files and directories and show the expected output. Relative paths are interpreted from the working directory, which you can inspect with [`pwd`](~peios/listing-and-paths/pwd):

```
$ pwd -P
/home/jack/project
```

In a script, check the command's exit status before using its output. The examples omit `realpath -q`: affected builds can report success for invalid operands in quiet mode. See the precise [compatibility warning and workaround](~peios/listing-and-paths/realpath#exit-status). Nonempty output alone does not establish success.

## Read a link's immediate target

Use plain [`readlink`](~peios/listing-and-paths/readlink) when you want the path stored in the link:

```
$ readlink "latest"
notes.txt
```

That is the immediate target, not an absolute path or a check that the target exists. Plain `readlink "notes.txt"` would fail because `notes.txt` is an ordinary file; that failure does not prove the file is absent.

## Require an existing input

Use `realpath -e` when every component, including the final file, must exist:

```
$ realpath -e "latest"
/home/jack/project/notes.txt
```

Unlike plain `readlink`, this resolves the whole path, following symbolic links and removing `.` and `..`. It also accepts an ordinary file. Under the example assumptions, `realpath -e "report.txt"` would fail because that final file is absent.

## Name a new file under existing directories

Default `realpath` allows the final component to be absent, but requires its parents to exist:

```
$ realpath "report.txt"
/home/jack/project/report.txt
```

Choose this when preparing an output filename in an existing directory. Resolving the name does not create the file or establish that a later write will succeed.

## Plan a path with missing parents

Use `-m` when parent directories may be absent too:

```
$ realpath -m "planned/drafts/report.txt"
/home/jack/project/planned/drafts/report.txt
```

Where the path cannot be walked, it is canonicalised as text. This gives you a planned spelling; it does not create directories or establish access. Default `realpath` would fail for this example because the parent directories are absent.

## Split a name or check it

If you only need part of a path, [`basename`](~peios/listing-and-paths/basename) and [`dirname`](~peios/listing-and-paths/dirname) operate on text without consulting the file system:

```
$ basename "planned/drafts/report.txt"
report.txt
```

```
$ dirname "planned/drafts/report.txt"
planned/drafts
```

[`pathchk`](~peios/listing-and-paths/pathchk) checks names instead: its default checks include file-system length limits and whether leading directories can be searched. A successful check does not guarantee that a future write will succeed.

For relative output, other link-handling modes, and full option tables, use the [`realpath` reference](~peios/listing-and-paths/realpath).
