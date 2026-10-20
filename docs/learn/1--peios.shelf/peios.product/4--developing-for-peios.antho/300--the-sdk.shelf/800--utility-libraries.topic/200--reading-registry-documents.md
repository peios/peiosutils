---
title: Reading Registry Documents
type: how-to
description: Parse a registry batch document in a C application without changing the registry.
---

Use `libreg` to read a registry batch document without applying it. This is
useful for an importer that shows a document before asking the user to apply
its changes.

## Before you start

- Install matching `libreg` and `libreg-devel` packages.
- Install a C compiler and `pkg-config`.
- Supply a UTF-8 JSON document in the registry batch format.

## Parse the document

1. Include `peios/reg.h` and create a span containing the JSON bytes. This
   example uses an empty batch, so it needs no live registry access:

   ```c
   #include <peios/reg.h>
   #include <stdio.h>

   int main(void) {
       const char json[] = "{\"keys\":[]}";
       peios_reg_bytes input = {
           .data = (const uint8_t *)json,
           .len = sizeof(json) - 1,
       };
       peios_reg_document *document = peios_reg_document_parse(input);
       int error = peios_reg_document_error(document);
       if (error == 0) {
           peios_reg_bytes output = peios_reg_document_json(document);
           fwrite(output.data, 1, output.len, stdout);
           putchar('\n');
       }
       peios_reg_document_free(document);
       return error == 0 ? 0 : 1;
   }
   ```

2. Save the example as `read-document.c`, then compile it against the
   installed library:

   ```sh
   cc read-document.c -o read-document $(pkg-config --cflags --libs reg)
   ```

## Check the result

Run `./read-document`. It prints the empty batch and exits successfully:

```json
{"keys":[]}
```

The handle owns the returned JSON bytes. The example writes them before
freeing the handle. For an application that retains the document in another
format, copy those bytes while the handle is still alive.

See `libreg(3)` for export, apply, errors, and lifetime requirements. Parsing
alone doesn't check whether you have permission to apply a document.
