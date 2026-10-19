---
title: Encoding Bytes as Printable Text
type: how-to
description: >-
  Round-trip a small byte stream through Base64 without treating encoding as
  secrecy.
related:
  - peios/hashing-and-encoding/basenc
  - peios/hashing-and-encoding/overview
---

Base32 and Base64 represent bytes using printable characters. They are useful
when an interface cannot carry arbitrary binary data directly.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Create the example input:

   ```sh
   printf 'Aster\n' > original.bin
   ```

2. Run the next command:

   ```sh
   base64 original.bin > encoded.txt
   ```

3. Run the next command:

   ```sh
   base64 -d encoded.txt > decoded.bin
   ```

4. Read the selected file contents:

   ```sh
   cat decoded.bin
   ```

## Check the result

The decoded output is `Aster` followed by a newline, matching the original.

## Check decoding before using the result

Encoding does not encrypt or authenticate the data. Anyone with the text can
decode it. Base32 uses a different alphabet and expansion ratio, but serves
the same transport purpose.

A decoder can write a valid prefix before reporting malformed input. Check its
exit status and keep partial output separate from a destination that another
program might consume. Do not replace an accepted artifact until its complete
decode and validation succeed.

See `base32(1)` and `base64(1)` for wrapping and input-handling options.
