---
title: Finding a Setting's Meaning
type: concept
description: >-
  Use installed registry manuals before reading or changing live
  configuration.
related:
  - peios/registry/reg
---

`regman` reads the documentation shipped for registry settings. It explains
what a key or value means, what values are accepted, and which component owns
it.

## Documentation and state answer different questions

A documented default is not necessarily the value configured on this machine.
`regman` does not query the live registry. Use `reg` for the effective value
and its available provenance.

The component that consumes a setting decides whether it is meaningful.
Successfully storing a value does not establish that the consumer accepted it.
Read the setting's type, range, and application behavior first.

## Start with a name or a search term

An exact key and value lookup is useful when an error names the setting. A key
lookup groups its documented values, while a search finds names and summaries
when you know the subject but not its location.

Some components document a family of keys using wildcard paths. An exact
record is more specific than a matching family description. Read the resolved
path so that you know which documentation answered the query.

## Installed packages supply the corpus

Packages place their registry manuals under `/usr/share/regman`. A missing
entry can mean that documentation is absent; it does not establish that the
live key is absent. Overlapping definitions are reported rather than silently
treated as one authoritative record.

The lookup index is an accelerator. Invalid indexes fall back to reading the
manuals instead of serving unchecked index data. Authors can format, lint, and
rebuild the index as part of preparing a package.

`regman(1)` defines lookup, search, path selection, and authoring commands.
