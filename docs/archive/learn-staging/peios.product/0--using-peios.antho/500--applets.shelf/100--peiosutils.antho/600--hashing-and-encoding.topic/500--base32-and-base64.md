---
title: base32 and base64
type: reference
description: Encode binary data as text using the base32 or base64 alphabet, and decode it back.
related:
  - peios/hashing-and-encoding/basenc
  - peios/hashing-and-encoding/overview
---

`base32` and `base64` encode binary data as plain text — and decode it back. They are the same command with one difference: the **alphabet** they use.

```
base32 [options] [file]
base64 [options] [file]
```

```
$ echo Peios | base64
UGVpb3MK
$ echo UGVpb3MK | base64 -d
Peios
```

## What encoding is for

Some channels only carry text safely — they mangle or reject arbitrary bytes. Encoding rewrites binary data using a small, safe set of characters, so it can pass through unharmed; decoding reverses it exactly. `base64` uses 64 characters and is the more compact; `base32` uses 32. Preserve the encoded text exactly, including letter case: the current Base32 decoder expects uppercase letters. With no file, both read standard input.

Encoding is **not** encryption — anyone can decode the result. See the [overview](~peios/hashing-and-encoding/overview).

## Options

By default these commands *encode*. The options below apply identically to both.

| Option | Effect |
|---|---|
| `-d`, `--decode` | Decode instead of encode. |
| `-i`, `--ignore-garbage` | When decoding, skip any characters that are not part of the alphabet, rather than failing on them. |
| `-w`, `--wrap=COLS` | When encoding, wrap the output to lines of `COLS` characters. The default is 76; `-w 0` disables wrapping and produces one unbroken line. |

When decoding, line-feed and carriage-return characters in the input are tolerated; `-i` is for *other* stray characters. Use it only when those characters are known transport noise. It discards characters outside the alphabet; it does not repair a damaged payload or normalize letter case.

## Decode before using the result

A decoder can write part of the result before reporting invalid input. A nonempty output file is therefore not proof that decoding succeeded. Decode to a separate file, check the exit status, and use the result only on success. Do not redirect over your input or an existing file you need: the shell opens and truncates the output before decoding starts.

For example, in a directory you control, choose a new output name and run:

```sh
if base64 --decode payload.b64 > payload.decoded; then
    printf '%s\n' 'Decoding succeeded; payload.decoded is ready to inspect.'
else
    printf '%s\n' 'Decoding failed; do not use payload.decoded.' >&2
    false
fi
```

In a script, put any processing of the decoded file inside the success branch. The failure branch returns a nonzero status and deliberately leaves any partial file for inspection; remove it before treating that name as a completed result. The same status check applies to `base32` and [`basenc`](~peios/hashing-and-encoding/basenc). Decoding success establishes a valid encoding, not the origin or safety of the decoded content.

This failure behavior is checked against [current peiosutils source](https://github.com/peios/peiosutils/blob/ec00f31b50f63e1084b6799037e3455049370c69/src/uu/base_common/src/lib.rs#L166-L219), not tested on an installed Peios release.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A file could not be read, or — when decoding — the input was not valid for the alphabet. |
