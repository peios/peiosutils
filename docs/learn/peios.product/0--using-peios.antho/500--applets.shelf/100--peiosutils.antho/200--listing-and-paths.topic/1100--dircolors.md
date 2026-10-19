---
title: Configuring Listing Colors
type: how-to
description: >-
  Generate the shell settings used by colored directory listings.
related:
  - peios/listing-and-paths/ls
  - peios/listing-and-paths/overview
---

`dircolors` produces an `LS_COLORS` setting for colored listings. It maps file
kinds and name patterns to terminal color sequences.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
dircolors -b
```

## Check the result

The output contains shell assignments for `LS_COLORS`. Printing them does not
change your shell's environment.

## Apply trusted configuration

Choose output syntax that matches your shell. Review generated assignments
before adding them to a shell configuration. Treat a custom configuration as
input from a trusted source rather than evaluating arbitrary text.

Color is a visual hint, not an authorization or file-content check. Keep
listings understandable without color, especially when sharing output or
sending it to a noninteractive consumer.

See `dircolors(1)` for the complete options and status contract.
