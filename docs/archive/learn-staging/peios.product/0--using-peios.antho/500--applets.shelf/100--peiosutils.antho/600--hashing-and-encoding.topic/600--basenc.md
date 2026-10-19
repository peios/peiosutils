---
title: basenc
type: reference
description: The general encoder — encode and decode data in base64, base32, base16, and several other encodings.
related:
  - peios/hashing-and-encoding/base32-and-base64
  - peios/hashing-and-encoding/overview
---

`basenc` is the **general** encoding command. Where [`base32` and `base64`](~peios/hashing-and-encoding/base32-and-base64) each do one encoding, `basenc` does many — the encoding is chosen by an option.

```
basenc encoding [options] [file]
```

```
$ echo Peios | basenc --base16
5065696F730A
```

Every run of `basenc` must name an encoding. With no file, it reads standard input.

## The encodings

| Option | Encoding |
|---|---|
| `--base64` | Standard base64 — the same as the `base64` command. |
| `--base64url` | Base64 with a file- and URL-safe alphabet. |
| `--base32` | Standard base32 — the same as the `base32` command. |
| `--base32hex` | Base32 with the extended-hex alphabet. |
| `--base16` | Hexadecimal. |
| `--base2msbf` | A bit string, most-significant bit first. |
| `--base2lsbf` | A bit string, least-significant bit first. |
| `--z85` | A compact, ASCII85-style encoding. When encoding, the input length must be a multiple of 4; when decoding, a multiple of 5. |
| `--base58` | Base58 — an alphabet chosen so its characters are not visually confusable. |

## Options

By default `basenc` *encodes*. These options apply to whichever encoding you chose:

| Option | Effect |
|---|---|
| `-d`, `--decode` | Decode instead of encode. |
| `-i`, `--ignore-garbage` | When decoding, skip characters that are not part of the alphabet. |
| `-w`, `--wrap=COLS` | When encoding, wrap output to lines of `COLS` characters. `-w 0` disables wrapping. |

## Check decoding before using the output

Select the same encoding used by the sender. Decoding can write a partial result before failing, so check the exit status before using the output. Follow the [separate-output example](~peios/hashing-and-encoding/base32-and-base64#decode-before-using-the-result), replacing `base64 --decode` with, for example, `basenc --base64url --decode` for URL-safe Base64 input. Do not use `--ignore-garbage` to guess your way past a damaged payload: it only discards characters outside the selected alphabet.

## `basenc` and the dedicated commands

For plain base64 or base32, `base64` and `base32` are the shorter way to ask. Use `basenc` when you need an encoding the dedicated commands do not offer — hex, URL-safe base64, the bit-string forms, z85, or base58.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | No encoding was named, a file could not be read, or the input was not valid for the chosen encoding. |
