---
title: Integrity Checks and Text Encodings
type: concept
description: >-
  Distinguish a content digest from a reversible encoding and from
  authenticated provenance.
related:
  - peios/hashing-and-encoding/checksum-commands
  - peios/hashing-and-encoding/basenc
---

A digest summarizes bytes for comparison. An encoding represents bytes in
another alphabet so that an interface can carry them. Neither operation by
itself proves who produced the data.

## Choose comparison or representation

Use these guides for the intended task:

- [Checking a recorded digest](~peios/hashing-and-encoding/checksum-commands) detects changed bytes.
- [Selecting an algorithm](~peios/hashing-and-encoding/cksum) makes the digest choice explicit.
- [Reading a legacy checksum](~peios/hashing-and-encoding/sum) explains compatibility limits.
- [Encoding printable text](~peios/hashing-and-encoding/base32-and-base64) carries arbitrary bytes.
- [Choosing an encoding alphabet](~peios/hashing-and-encoding/basenc) matches a transport format.

## Authenticate the expected value

A checksum list is useful only relative to its source. Replacing both the file
and its expected digest defeats a comparison. Obtain expected values through
an authenticated channel or verify an appropriate signature.

A decoder can produce partial bytes before rejecting malformed input. Check
its status and validate the complete decoded result before publishing or
executing it.
