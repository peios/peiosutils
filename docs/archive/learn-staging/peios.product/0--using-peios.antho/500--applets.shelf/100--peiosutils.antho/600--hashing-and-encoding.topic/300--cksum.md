---
title: cksum
type: reference
description: The general checksum tool — compute or verify a digest with any of the supported algorithms.
related:
  - peios/hashing-and-encoding/checksum-commands
  - peios/hashing-and-encoding/overview
---

`cksum` is the **general** checksum command. Where each of the [checksum commands](~peios/hashing-and-encoding/checksum-commands) is fixed to one algorithm, `cksum` does any of them — the algorithm is an option.

```
cksum [options] [file...]
```

```
$ cksum installer.iso
3915528286 372736000 installer.iso
$ cksum -a sha256 --untagged installer.iso
9f86d0818884...b1a5  installer.iso
```

With no `-a`, `cksum` computes a CRC checksum and prints the CRC value, the file's byte count, and the name. With `-a`, it uses the selected algorithm. The SHA-256 example adds `--untagged` to request the plain `hash  file` output; without it, SHA-256 output is tagged by default.

## Choosing the algorithm

| Option | Effect |
|---|---|
| `-a`, `--algorithm=NAME` | Use the named algorithm. |

`NAME` may be:

| Name | Algorithm |
|---|---|
| `crc` | The default CRC. |
| `crc32b` | A CRC-32 variant. |
| `md5` | MD5 — as `md5sum`. |
| `sha1` | SHA-1 — as `sha1sum`. |
| `sha224`, `sha256`, `sha384`, `sha512` | The SHA-2 family — as the matching `sha*sum`. |
| `sha3` | SHA-3. |
| `blake2b` | BLAKE2b — as `b2sum`. |
| `sm3` | The SM3 hash. |
| `sysv`, `bsd` | The legacy block checksums — as [`sum`](~peios/hashing-and-encoding/sum). |

`crc`, `crc32b`, `sha3`, and `sm3` are available *only* through `cksum` — there is no dedicated command for them.

## Verifying

Choose the check command to match the publisher's manifest format. Run it from the directory against which the manifest's file names were written; the manifest's own location does not set that directory. The [checksum workflow](~peios/hashing-and-encoding/checksum-commands#verifying-with-a-checksum) covers trusted expected values, complete manifest lines and exit-status checks.

For tagged lines such as `SHA256 (installer.iso) = ...`, the input identifies the algorithm:

```
cksum --check --strict CHECKSUMS
```

For untagged SHA-256 lines such as `hash  installer.iso`, supply the algorithm:

```
cksum --algorithm=sha256 --check --strict SHA256SUMS
```

The dedicated `sha256sum --check --strict SHA256SUMS` command already selects SHA-256. With `cksum`, an explicitly selected algorithm must agree with any tagged input. Do not add `--tag`, `--binary` or `--text` to a check command: those output-mode options are rejected during verification.

You cannot combine `--check` with `--algorithm=crc`, `crc32b`, `bsd` or `sysv`. The default CRC output is not a checkable checksum manifest. Use normal newline-delimited checksum output for manifests; `--zero` output is not read as NUL-delimited check input.

| Option | Effect |
|---|---|
| `-c`, `--check` | Read checksums and verify the files against them. |
| `--ignore-missing` | Skip unavailable listed files when intentionally checking a subset; see the caveat below. |
| `--quiet` | Print nothing for files that pass. |
| `--status` | Suppress comparison results and summary warnings; check the exit status. File-access or argument errors may still print diagnostics. |
| `-w`, `--warn` | Warn about malformed checksum lines. |
| `--strict` | Fail on a malformed checksum line. |

Without `--strict`, malformed lines can be skipped while a check still succeeds. A checksum input with no properly formatted lines fails in either mode. Use `--ignore-missing` only for an intentional subset: the current implementation skips file-open failures beyond missing files, so success does not establish that the whole set was available. It still fails if no file in a checksum input verifies successfully.

## Output format

| Option | Effect |
|---|---|
| `--tag` | Produce a tagged `ALGORITHM (file) = hash` line. The default for most algorithms. |
| `--untagged` | Produce a plain `hash  file` line. |
| `--raw` | Output the raw binary digest, with nothing else. |
| `--base64` | Print the digest in base64 rather than hexadecimal. |
| `-l`, `--length=BITS` | For an algorithm that supports it, produce a shorter digest. |
| `-z`, `--zero` | End each output line with a NUL character. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success under the selected options. |
| `1` | Verification failed, a file could not be accessed or read, or input/options were invalid. Malformed checksum lines cause failure with `--strict`, or when no properly formatted lines exist. |

These verification rules are checked against [current command handling](https://github.com/peios/peiosutils/blob/ec00f31b50f63e1084b6799037e3455049370c69/src/uu/checksum_common/src/lib.rs#L141-L209) and [manifest validation](https://github.com/peios/peiosutils/blob/ec00f31b50f63e1084b6799037e3455049370c69/src/uucore/src/lib/features/checksum/validate.rs#L615-L1019), not tested on an installed Peios release. Treat any nonzero status as failure.
