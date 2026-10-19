---
title: Hashing and encoding
type: reference
description: The commands that compute checksums and hashes to verify data integrity, and that encode binary data into text and back.
related:
  - peios/hashing-and-encoding/checksum-commands
  - peios/hashing-and-encoding/basenc
---

This topic covers two related jobs. **Hashing** reduces a file to a short fixed-size value — a checksum — that can be used to tell whether the file has changed. **Encoding** rewrites binary data as plain text so it can travel safely through channels that only handle text.

## The commands

**Checksums and hashes**

| Command | Purpose |
|---|---|
| [checksum commands](~peios/hashing-and-encoding/checksum-commands) | `md5sum`, `sha1sum`, `sha256sum`, and the rest — one command per hash algorithm. |
| [`cksum`](~peios/hashing-and-encoding/cksum) | The general checksum tool — any of the algorithms, selected by an option. |
| [`sum`](~peios/hashing-and-encoding/sum) | A small, legacy block-checksum. |

**Encoding**

| Command | Purpose |
|---|---|
| [`base32` and `base64`](~peios/hashing-and-encoding/base32-and-base64) | Encode binary data as text using the base32 or base64 alphabet, and decode it back. |
| [`basenc`](~peios/hashing-and-encoding/basenc) | The general encoder — base64, base32, base16, and several more, selected by an option. |

## What these commands are not

It is worth being clear up front, because both jobs are easy to mistake for something they are not.

**A hash is not encryption.** Hashing is one-way: a checksum tells you *whether* data has changed, but the original cannot be recovered from it. Comparing it with a trusted expected hash can detect corruption or tampering; a hash supplied alongside an untrusted file does not establish who published that file. Hashing does not keep anything secret.

**Encoding is not encryption either.** `base64` rewrites data into a text-safe form, but anyone can decode it straight back — it is a change of *representation*, not of secrecy. Encoding something does not protect it.

Neither hashing nor encoding hides data. They are about *integrity* and *transport*, not confidentiality.

## Where to start

To verify a file you downloaded against a published checksum, read the [checksum commands](~peios/hashing-and-encoding/checksum-commands). To turn binary data into something safe to paste into text, read [`base32` and `base64`](~peios/hashing-and-encoding/base32-and-base64).
