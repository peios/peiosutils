---
title: Utility Libraries
type: concept
description: Shared libraries for registry documents, registry manuals, and event catalogs.
---

The Peios utility libraries let your application work with the same document
formats as the command-line tools. You can import a registry document, show
configuration help, or look up an event description without starting another
process or interpreting terminal output.

## Choosing a library

| Library | Purpose | Reference |
|---|---|---|
| `libreg` | Parse, export, and apply registry batch documents | `libreg(3)` |
| `libregman` | Parse and search registry manuals | `libregman(3)` |
| `libevman` | Parse, check, and search event catalogs | `libevman(3)` |

Each library has a C interface and a safe Rust wrapper. The runtime package
provides a shared library; its matching development package provides headers
and linking information. The libraries share Peiosutils release versions,
while their ABI major identifies the compatible C interface.

Parsing a registry document doesn't change the registry. Applying it is a
separate operation that uses the caller's access token. A document that parses
successfully can still fail validation or access checks when applied.

Manual and event parsers can return usable records alongside diagnostics.
Check both: a successful library operation doesn't mean every source record
is valid. This lets an editor display the valid content and explain problems
in the same view.

## Ownership across language boundaries

A C operation returns a handle that owns its result or its error. Check the
error accessor, then read the result. Free the handle on either path. Views
returned by the library borrow that handle, so copy anything you need to
retain before freeing it.

The Rust wrappers manage these handles automatically. Registry-manual records
and event-query matches are owned Rust values. A registry document keeps its
library handle alive while you borrow its JSON representation.

The C interfaces accept pointer-and-length spans rather than terminated
strings. This preserves the difference between an absent optional value and
an explicitly empty one. Text is UTF-8; filesystem paths in the manual and
event libraries retain their original Unix bytes.

## Moving from the earlier Rust crates

Use `peios-reg`, `peios-regman`, and `peios-evman` for application code. They
call the corresponding shared library. The earlier Rust-only crates exposed
implementation types directly; changing only a dependency name doesn't adapt
those types to the new ownership model.

The implementation crates `registry-docs`, `registry-manual`, and
`event-catalogue` are internal to Peiosutils. Keep application integrations on
the public wrappers or C interfaces so an implementation change doesn't
require a matching change in every consumer.
