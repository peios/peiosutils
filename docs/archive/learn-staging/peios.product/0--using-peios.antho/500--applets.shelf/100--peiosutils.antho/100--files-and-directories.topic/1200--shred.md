---
title: shred
type: reference
description: Destroy a file's contents by overwriting them repeatedly, so they cannot be recovered — including how --force overrides a file's protection.
related:
  - peios/files-and-directories/rm
  - peios/security-descriptors/overview
  - peios/files-and-directories/overview
---

`shred` destroys a file's contents. It does not just unlink the file — it **overwrites the data**, repeatedly, so that what was there cannot be read back even with effort and specialised equipment.

```
shred [options] file...
```

```
$ shred -u -v secret-keys.txt
```

[`rm`](~peios/files-and-directories/rm) removes a file's *name*; the data may sit on the device, recoverable, until something else happens to overwrite it. `shred` overwrites the data on purpose. Use it when a file held something that must not be recoverable.

By default `shred` overwrites a file three times and then **leaves the file in place** — empty of its old contents but still present. That default exists because `shred` is often pointed at device files, which usually should not be removed. To remove the file after shredding it, pass `-u`.

## When shredding actually works

`shred` relies on one assumption: that writing to the file overwrites the *same physical storage* the old data occupied. On a traditional file system that holds. Several common file system designs break it — anything that writes new data to a fresh location instead of in place, keeps snapshots, journals file data, caches copies elsewhere, or compresses. On those, `shred` cannot guarantee the old data is gone.

Backups and remote mirrors are a separate matter entirely: `shred` cannot reach a copy of the file that lives somewhere else.

Treat `shred` as effective on a plain local file system and uncertain otherwise.

## Options

| Option | Effect |
|---|---|
| `-n`, `--iterations=N` | Overwrite `N` times instead of the default of 3. |
| `-u`, `--remove[=HOW]` | Remove the file after overwriting it. `HOW` controls how thoroughly the *name* itself is obscured before the file is unlinked. |
| `-z`, `--zero` | Add a final pass that writes zeros, so the file does not visibly look shredded afterwards. |
| `-s`, `--size=N` | Shred `N` bytes, rather than the whole file. Accepts size suffixes such as `K`, `M`, `G`. |
| `-x`, `--exact` | Do not round the shredded size up to the next full block. Without it, `shred` rounds up so the block's slack space is overwritten too. |
| `--random-source=FILE` | Take the random bytes for the overwrite passes from `FILE`. |
| `-v`, `--verbose` | Show progress as each pass runs. |
| `-f`, `--force` | Override a file's protection: when a live access check shows you cannot write the file, rewrite its security descriptor so the overwrite can proceed (see below). |

## `--force` and a file's protection

To overwrite a file, `shred` has to be able to write it. If the file's [security descriptor](~peios/security-descriptors/overview) does not grant you write access, an ordinary `shred` cannot proceed.

`-f` (`--force`) handles that case. When `--force` is given and a live access check shows you cannot currently write the file, `shred` **rewrites the file's security descriptor** to one that grants full access, and then overwrites the file. The reasoning is deliberate: the file is about to be destroyed anyway, and `--force` is you saying "override this file's protection."

`--force` can do this only when you are allowed to change the file's security in the first place — that is, when you own the file or hold the right to rewrite its access rules. If you cannot change the descriptor, `--force` cannot grant itself write access, and the shred still fails. `--force` overrides a file's *protection*; it does not manufacture authority you do not have.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every file was shredded successfully. |
| `1` | A file could not be shredded. |
