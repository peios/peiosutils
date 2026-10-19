---
title: Choosing a Binary-to-Text Encoding
type: how-to
description: >-
  Use one encoding selector for a transport format that requires a specific
  alphabet.
related:
  - peios/hashing-and-encoding/base32-and-base64
  - peios/hashing-and-encoding/overview
---

`basenc` supports several binary-to-text encodings. Choose the encoding
required by the receiving interface rather than guessing from the text's
appearance.

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
   basenc --base64url original.bin > encoded.txt
   ```

3. Run the next command:

   ```sh
   basenc --base64url -d encoded.txt > decoded.bin
   ```

4. Read the selected file contents:

   ```sh
   cat decoded.bin
   ```

## Check the result

The decoded file contains `Aster` followed by a newline.

## Use the same encoding in both directions

Base64 and its URL-safe alphabet differ even when their output looks similar.
Encoding is not encryption, and successful decoding does not establish trust
in the result.

Check decoding status before accepting the output. A failing decode can leave
a partial output file. `basenc(1)` lists the supported alphabets, wrapping
rules, and validation options.
