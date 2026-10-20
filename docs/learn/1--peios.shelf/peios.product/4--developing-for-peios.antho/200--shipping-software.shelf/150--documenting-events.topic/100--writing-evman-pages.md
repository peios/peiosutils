---
title: Writing evman Pages
type: how-to
description: Define and validate the event catalog fragment shipped by your component.
---

Ship an `.evman` fragment with each component that emits events. The fragment
defines its event types and fields so operators and consumers can interpret
the stream. The [event catalog specification](~specifications/pgss/events/the-catalogue)
owns the format and its requirements.

## Before you start

- Install `evman` from Peiosutils.
- Identify the event types, tiers, and fields your component emits.
- Obtain the catalog fragments that define the fields your events reference.
- Follow [regman and evman prose style](~psc/regman-and-evman).

## Choose field definitions

1. Search the existing vocabulary before defining a field:

   ```sh
   evman object.file
   evman -k snapshot
   ```

2. Reuse a field when its meaning fits. Define a package-owned field under
   its role and package name only when the shared vocabulary lacks that
   meaning.
3. Keep each definition in its owning component's fragment. Other fragments
   carry the field by reference rather than defining it again.

A `--- field` record defines a field. A `field:` line in an event describes
its presence in that event. Two definitions of one field are an error.
Related fields can be collected in a `group` record with `include:` lines;
an event can then include the group.

## Describe an event

Create `org.example.backup.evman` for this illustrative backup component:

```text
--- field object.org.example.backup.snapshot.id
type: bin.guid

The snapshot an event concerns, by the GUID the backup tool assigned it.
Stable for the life of the snapshot, including verification and deletion.

--- event org.example.backup.snapshot.created
tier: standard

A snapshot was written, or an attempt to write one failed.

field: object.kind                            required
  Always `file`.
field: object.file.path                       required
  The snapshot archive.
field: object.file.size                       when outcome.success == true
field: object.org.example.backup.snapshot.id  required
field: operation.duration                     required
field: outcome.success                        required
field: outcome.reason                         when outcome.success == false
  values: disk-full | source-unreadable | cancelled
  closed: false
  Why the snapshot was not written.
field: outcome.errno                          optional
```

The first sentence of each body is its summary. Explain when an event is
emitted, when it is not, and how to interpret its fields. Use `**bold**` and
code spans for emphasis and literal names.

Presence is `required`, `optional`, or `when` followed by a condition. An
indented paragraph after a field is its event-specific explanation. That
explanation can narrow the general definition but cannot contradict it.
Use `gating` to describe selection of occurrences and `cardinality` when one
occurrence produces multiple records; follow the specification for the full
header set, including `asserted` and `carried`.

Enumeration definitions declare `values` and `closed`. String enumeration
values use kebab case. Integer enumerations pair each number with its display
name, such as `0 None | 512 Protected`. Event-specific enumerations, including
`outcome.reason`, declare their values on the event's field entry.

Set `closed: true` only when consumers can rely on the set being complete.
Check the emitted values against the declaration; a parser cannot establish
that runtime behavior matches the catalog.

## Validate and preview

1. Create a disposable directory and copy the candidate fragment plus the
   fragments it references into it. Include `kernel.evman` for the shared
   fields in this example. Do not also copy an installed version of your
   candidate fragment.

   ```sh
   preview_dir=$(mktemp -d)
   cp /usr/share/evman/kernel.evman "$preview_dir/"
   cp org.example.backup.evman "$preview_dir/"
   ```

2. Check the combined definitions and inspect the event:

   ```sh
   evman lint "$preview_dir"/*.evman
   EVMAN_DIR="$preview_dir" evman org.example.backup.snapshot.created
   rm -r -- "$preview_dir"
   ```

3. Repeat lint against the full intended package set before release, replacing
   the installed version of your fragment with the candidate. This catches
   duplicate definitions and incompatible names across components.

A successful lint run prints no findings. Findings identify the file, line,
and catalog rule. Lint checks the declared catalog; separately test actual
events for presence conditions, value ranges, tiers, and emission behavior.

## Package the fragment

Include the fragment at `usr/share/evman/org.example.backup.evman` in the
component's package payload. No central registration or installation hook is
needed. Removing the package removes its fragment.

The package owns the definitions throughout their lifetime. Renaming or
changing their meaning can affect filters and consumers; review such changes
against the event specification before publishing them.
