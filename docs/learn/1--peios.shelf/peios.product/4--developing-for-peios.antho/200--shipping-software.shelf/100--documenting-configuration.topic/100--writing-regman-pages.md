---
title: Writing regman Pages
type: how-to
description: Write and check registry documentation that ships with the component that owns the keys.
---

Ship a `.regman` fragment with your component so an operator can look up
its registry keys and values with `regman`. Keep the fragment beside the
code that reads those values, and update both together.

## Before you start

- Install `regman` from Peiosutils.
- Identify the keys your component owns and the values it reads.
- Follow [regman and evman prose style](~psc/regman-and-evman).

Document another component's key by referring to its full path in your
record's body. Do not publish a second definition of that key.

## Write a fragment

1. Create a file named for your component, such as `exampled.regman`.
2. Add a key record explaining the subtree, then a record for each value.
3. Write a standalone summary as the first nonempty body line. Explain the
   effect, trade-offs, dependencies, and unusual cases in the remaining body.

This example describes an illustrative spooler and its queue limit:

```text
--- machine\system\exampled
canonical: Machine\System\Exampled

Tuning parameters for the Exampled spooler.

--- machine\system\exampled maxqueuedepth
canonical: Machine\System\Exampled MaxQueueDepth
type: REG_DWORD
default: 1024
valid: 16–65536
applies: restart

Maximum number of jobs held in the spool queue before submissions are rejected.

Increase the limit to absorb longer bursts. Each queued job consumes memory.
The spooler reads this value when it starts.
```

The fence starts a record. Header lines continue until the first blank
line, and the body continues until the next fence. `canonical` supplies the
human-readable registry path and, for a value, its name after a space.
`regman fmt` derives the anchor from this header through Unicode simple
case folding. Do not maintain the folded anchor by hand.

A record with any `type`, `default`, `valid`, or `applies` header is a value
record. Supply all four for a useful value description. A record without
those headers describes a key. `deprecated` adds a deprecation notice;
explain the replacement and its limitations in the body.

Header names are case-insensitive, and surrounding whitespace is trimmed.
Unknown headers are ignored, so a typo can silently remove information.
A record without `canonical` is invalid and cannot provide a lookup result.
Check the rendered record as well as the lint result.

## Format and check

1. Format and lint the fragment:

   ```sh
   regman fmt exampled.regman
   regman lint exampled.regman
   ```

2. Preview it from the directory containing only the fragments under review:

   ```sh
   REGMAN_DIR="$PWD" regman 'Machine\System\Exampled'
   REGMAN_DIR="$PWD" regman 'Machine\System\Exampled' MaxQueueDepth
   ```

3. Compare every header with the implementation. Confirm the allowed type,
   bounds, behavior when absent, and point at which a changed value takes
   effect.

Formatting updates anchors without rewriting the body, preamble, or line
endings. Running it twice should produce the same file. Lint checks record
structure and anchors; it cannot prove that the program implements the prose.

`default` describes the component's behavior, not the value on the machine
running `regman`. Use `live`, `restart`, or `reboot` for `applies`, with a
short parenthetical qualification when needed.

## Package the fragment

1. Include `exampled.regman` in the package payload as
   `usr/share/regman/exampled.regman`.
2. Install it with the component that owns the documented keys.
3. Check the installed lookup before releasing the package.

The filename stem identifies the provider in lookup results. Installation
requires no registration hook. An absent documentation directory means no
fragments, not a broken registry. If an installation mechanism replaces files
in place, use atomic replacement so readers do not see a partial fragment.

An index is an optional lookup optimization. The reader can scan fragments
when the index is absent, stale, or invalid. Do not make correctness depend
on generating an index during installation.

## Check the result

The key lookup shows the subtree summary and its values. The value lookup
shows its headers and body under the expected provider. A second provider
for the same record indicates overlapping ownership; resolve that overlap
before shipping.

For dynamic names, use [Registry Documentation Patterns](~peios/developing-for-peios/documenting-configuration/registry-documentation-patterns).
