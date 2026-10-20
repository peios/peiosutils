---
title: Verifying Checksums and Encoding Bytes
type: how-to
description: Record and verify file digests, choose a checksum algorithm, and encode bytes as Base64 or Base32 text, knowing what each operation does and doesn't prove.
related:
  - peios/working-with-text/text-streams
  - peios/certificate-trust/overview
---

A checksum, or digest, summarizes a file's bytes so that you can later check
whether they changed. An encoding represents bytes as printable text so that
they can pass through an interface that carries only text. Neither proves who
produced the data: that needs a trusted source for the expected value, or a
signature.

The examples use sample file names such as `release.txt`. Create those files
first, or substitute your own.

## Record and verify digests

Record a SHA-256 digest of a file, then check the file against it:

```sh
sha256sum release.txt > checksums.txt
sha256sum -c checksums.txt
```

The check reports each file and whether its bytes still match, and fails if
any doesn't. Anyone who can replace both the file and its checksum list can
make the check pass, so get the expected digest through a channel you trust,
or verify a signature with a trusted key.

`cksum` offers several algorithms through one command. Name the algorithm so
that a saved result says what it means:

```sh
cksum -a sha256 release.txt
```

The default `cksum` algorithm and the older `sum` checksum detect accidental
changes, such as transmission errors. They have too few possible values to
identify a file securely, so use a cryptographic digest such as SHA-256 for
new work.

## Encode and decode bytes

Encode a file as Base64 text, then decode it:

```sh
base64 original.bin > encoded.txt
base64 -d encoded.txt > decoded.bin
```

`base32` works the same way with a different alphabet. `basenc` selects among
several encodings, such as `--base64url`, the variant used in web addresses.
Use the encoding the receiving program expects: Base64 and its URL-safe form
look alike but aren't interchangeable.

Encoding isn't encryption: anyone with the text can decode it. A decoder can
write part of its output before it finds invalid input, so check its exit
status, and don't let another program use the output until the whole decode
has succeeded.

## Commands on this page

This page covers these commands:

| Command | Use it to | Manual |
|---|---|---|
| `sha256sum` and the other digest commands | Record and check file digests | `sha256sum(1)` |
| `cksum` | Calculate a checksum with a chosen algorithm | `cksum(1)` |
| `sum` | Calculate an older checksum another system requires | `sum(1)` |
| `base64`, `base32` | Encode bytes as text | `base64(1)`, `base32(1)` |
| `basenc` | Encode with a chosen alphabet | `basenc(1)` |
