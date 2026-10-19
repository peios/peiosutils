---
title: Checksum commands
type: reference
description: md5sum, sha1sum, sha256sum and the rest — the family of commands that each compute and verify one fixed hash algorithm.
related:
  - peios/hashing-and-encoding/cksum
  - peios/hashing-and-encoding/overview
---

This page covers a family of commands that all work the same way and differ only in **which hash algorithm** they use:

| Command | Algorithm | Digest size |
|---|---|---|
| `md5sum` | MD5 | 128-bit |
| `sha1sum` | SHA-1 | 160-bit |
| `sha224sum` | SHA-224 | 224-bit |
| `sha256sum` | SHA-256 | 256-bit |
| `sha384sum` | SHA-384 | 384-bit |
| `sha512sum` | SHA-512 | 512-bit |
| `b2sum` | BLAKE2b | up to 512-bit |

```
sha256sum [options] [file...]
```

Everything below is written with `sha256sum`, but applies to every command in the table.

## Computing a checksum

Run plainly, the command prints the hash of each file, followed by the file name:

```
$ sha256sum installer.iso
9f86d0818884...b1a5  installer.iso
```

With no file, it reads standard input. The hash is a fingerprint of the file's contents: change a single byte and the hash changes completely.

## Verifying with a checksum

The everyday use is **verification** — confirming a file is exactly what it should be, usually a download.

First, the file's publisher computes a checksum and publishes it, often in a file:

```
$ sha256sum installer.iso > installer.iso.sha256
```

Obtain the publisher's checksum file through a trusted source. Computing a new checksum from your own download only records its current contents; it does not verify them against the publisher's copy.

Use the complete digest-and-filename lines, not a bare hash or the abbreviated display example above. Run the check from the directory against which the manifest's file names were written. Giving an absolute path to the checksum file does not change that working directory.

For example, with both files in `~/Downloads`:

```
$ cd ~/Downloads
$ sha256sum --check --strict installer.iso.sha256
installer.iso: OK
```

`-c` reads each `hash  filename` line, recomputes the hash, and reports `OK` or `FAILED`. A checksum mismatch means the file differs from the one the checksum describes. File-access failures and malformed input can also prevent verification; read the diagnostic and check the exit status before using the download.

Keep manifests newline-delimited. `--zero` output is not a NUL-delimited input format for `--check`.

| Option | Effect |
|---|---|
| `-c`, `--check` | Read checksums from the given files and verify them. |
| `--ignore-missing` | In check mode, skip unavailable listed files when intentionally checking a subset; see the caveat below. |
| `--quiet` | In check mode, print nothing for files that pass — only failures. |
| `--status` | In check mode, suppress comparison results and summary warnings; determine success from the exit status. File-access or argument errors may still print diagnostics. |
| `-w`, `--warn` | Warn about improperly formatted lines in the checksum file. |
| `--strict` | In check mode, fail if any checksum line is malformed. |

Without `--strict`, malformed lines can be skipped while the command still succeeds. A checksum input with no properly formatted lines fails in either mode.

Avoid `--ignore-missing` when verifying that a complete download set is present. In the current implementation it skips file-open failures, including errors other than a missing file. It still fails if no file in a checksum input verifies successfully.

## Output format

These dedicated commands produce plain, untagged `hash  file` output by default. Unlike `cksum`, they do not accept `--untagged`.

| Option | Effect |
|---|---|
| `--tag` | Produce a tagged line — `SHA256 (file) = hash` — that records which algorithm was used. |
| `-z`, `--zero` | End each output line with a NUL character instead of a newline. |
| `-b`, `--binary` | Note the file as read in binary mode. |
| `-t`, `--text` | Note the file as read in text mode. |

`b2sum` additionally accepts `-l`, `--length=BITS` to produce a shorter BLAKE2b digest.

## A note on choosing an algorithm

These commands detect change — but not all of them resist a *deliberate* attempt to forge a match. MD5 and SHA-1 can be defeated by an attacker who wants two different files to share a checksum. They are still fine for catching accidental corruption, but for verifying that a file has not been **tampered with**, use a SHA-2 command (`sha256sum` and up) or `b2sum`.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success under the selected options. |
| `1` | Verification failed, a file could not be accessed or read, or input/options were invalid. Malformed checksum lines cause failure with `--strict`, or when no properly formatted lines exist. |

The flag and verification behavior above is checked against [peiosutils source at `ec00f31`](https://github.com/peios/peiosutils/blob/ec00f31b50f63e1084b6799037e3455049370c69/src/uu/checksum_common/src/lib.rs#L119-L209) and its [checksum validator](https://github.com/peios/peiosutils/blob/ec00f31b50f63e1084b6799037e3455049370c69/src/uucore/src/lib/features/checksum/validate.rs#L852-L1019). It was not tested on an installed Peios release. Treat any nonzero status as failure and use the diagnostics to distinguish a mismatch from an input or access error.
