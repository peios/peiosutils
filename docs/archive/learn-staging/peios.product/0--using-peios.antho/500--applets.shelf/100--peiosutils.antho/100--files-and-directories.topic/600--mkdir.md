---
title: mkdir
type: reference
description: Create directories — and the shared creation flags that set a new object's security descriptor, used by mkdir, mkfifo, mknod, and touch.
related:
  - peios/files-and-directories/mkfifo
  - peios/files-and-directories/mknod
  - peios/files-and-directories/touch
  - peios/security-descriptors/overview
  - peios/security-descriptors/inheritance
---

`mkdir` creates directories.

```
mkdir [options] directory...
```

```
$ mkdir reports
$ mkdir -p projects/2026/q2
```

## Creating parent directories

By default `mkdir` creates exactly the directories you name, and fails if a parent is missing. `-p` removes that restriction:

| Option | Effect |
|---|---|
| `-p`, `--parents` | Create any missing parent directories along the way. Also makes `mkdir` succeed quietly if the directory already exists. |
| `-v`, `--verbose` | Print a line for each directory as it is created. |

## The creation flags

A new directory needs a [security descriptor](~peios/security-descriptors/overview) — the record of who owns it and who may do what to it. By default, **a new directory inherits its descriptor from the directory it is created in**: it picks up the access rules its parent hands down. For most directories that is exactly right, and you pass no extra options at all.

When you need the new directory to have a *different* descriptor, the **creation flags** set it explicitly. These five flags are shared, unchanged, by `mkdir`, [`mkfifo`](~peios/files-and-directories/mkfifo), [`mknod`](~peios/files-and-directories/mknod), and [`touch`](~peios/files-and-directories/touch) — whenever one of those commands creates a new object, it accepts this same set. This page is the full reference for them.

| Flag | Argument | Effect |
|---|---|---|
| `--owner` | SID | Make this principal the owner, instead of you. |
| `--group` | SID | Set the object's group. |
| `--label` | level | Give the object a mandatory integrity label. |
| `--no-inherit` | — | Do **not** inherit rules from the parent directory; lock the object to its owner. |
| `--sddl` | SDDL string | Supply the entire security descriptor directly. |

A SID argument is either a literal `S-1-…` value or a well-known alias — `BA` for the built-in administrators, for example.

`--label` takes one of these levels, lowest to highest: `untrusted`, `low`, `medium`, `medium-plus`, `high`, `system`, `protected`. The label applies the standard rule for files and directories — a principal below the object's level cannot write to it.

### Inherited, or explicit

The default and `--no-inherit` are the two ends of a choice.

- **Inherited (the default)** — the new directory tracks its parent. Rules the parent hands down apply to it, and as the parent's rules change the directory follows along. This keeps a tree of directories consistent without per-directory effort.
- **`--no-inherit`** — the new directory takes nothing from its parent. Its access is locked to its owner, and it does not follow the parent's rules. Use it to carve out a directory whose security stands apart from the tree around it.

See [Inheritance](~peios/security-descriptors/inheritance) for the full model.

### `--sddl` for a complete descriptor

The four flags above are shortcuts for common adjustments. When you need to specify the *whole* descriptor — a particular set of access rules, audit rules, owner, and group all at once — `--sddl` takes it as a single string in the security-descriptor definition language:

```
$ mkdir --sddl 'O:BAG:BAD:(A;OICI;FA;;;BA)' secure-area
```

`--sddl` is mutually exclusive with `--owner`, `--group`, `--label`, and `--no-inherit` — it already says everything those flags would say, so combining them is rejected.

### How the descriptor is applied

`mkdir` creates the directory first and then applies the descriptor to it. The two steps are not separately visible — `mkdir` either finishes with the directory created and secured as asked, or fails as a whole. If applying the descriptor fails, the half-created directory is not left behind.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every directory was created. |
| `1` | A directory could not be created, or a creation flag could not be applied. |
