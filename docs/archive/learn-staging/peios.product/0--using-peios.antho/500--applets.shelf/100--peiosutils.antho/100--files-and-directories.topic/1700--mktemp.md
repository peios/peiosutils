---
title: mktemp
type: reference
description: Create a temporary file or directory with a safely unique name.
related:
  - peios/files-and-directories/touch
  - peios/files-and-directories/mkdir
  - peios/files-and-directories/overview
---

`mktemp` creates a temporary file — or, with `-d`, a temporary directory — gives it a name that is not already taken, and prints that name.

```
mktemp [options] [template]
```

```
$ mktemp
/tmp/tmp.QV8x3kZ1aR
$ mktemp -d
/tmp/tmp.7bNc0wPe2L
```

The reason to use `mktemp` rather than inventing a name yourself is **safety**. A script that picks a fixed name like `/tmp/work` has two problems: two copies of the script collide, and a name an attacker can predict is a name an attacker can interfere with before the script gets to it. `mktemp` chooses an unpredictable name *and* creates the file in the same step, so nothing can slip in between. It prints the name it used, for the script to capture:

```
work=$(mktemp)
# ...use "$work"...
rm "$work"
```

The temporary file or directory `mktemp` creates is a new object, and a new object needs a [security descriptor](~peios/security-descriptors/overview); it inherits one from the directory it is created in.

## Templates

The name comes from a **template** — a name with a run of trailing `X` characters, each of which `mktemp` replaces with a random character. More `X`s means a wider range of possible names.

```
$ mktemp report-XXXXXX.txt
report-a8Kp2Q.txt
```

With no template, `mktemp` uses a built-in default that places the file in the temporary directory. A template needs at least three `X`s.

## Options

| Option | Effect |
|---|---|
| `-d`, `--directory` | Create a directory instead of a file. |
| `-p`, `--tmpdir[=DIR]` | Create the temporary object inside `DIR`. With no `DIR`, use the directory named by the `TMPDIR` environment variable, or the system temporary directory. The template is then just the final name component. |
| `--suffix=SUFFIX` | Append `SUFFIX` after the random part of the name — for giving the file an extension. |
| `-u`, `--dry-run` | Do not create anything; just print a name that *would* be free. This reintroduces the race the command exists to avoid — use it only when you genuinely cannot create the object yet. |
| `-q`, `--quiet` | Print no error message if creation fails; rely on the exit status. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | The temporary file or directory was created; its name was printed. |
| `1` | Creation failed — a bad template, or a directory that does not exist. |
