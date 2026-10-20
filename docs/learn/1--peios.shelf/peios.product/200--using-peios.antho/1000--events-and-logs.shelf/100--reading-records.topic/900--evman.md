---
title: Finding an Event's Meaning
type: concept
description: >-
  Read event and field definitions before interpreting a captured record.
related:
  - peios/registry/regman
---

`evman` reads installed event documentation. It explains what an event records
and what its fields mean; it does not report which events occurred on this
machine.

## Read the event and the field together

An event definition describes when it is emitted and which fields it can
carry. A field definition describes the field's general type and meaning. An
event can add a narrower explanation for its use of that field.

Pay attention to required, optional, and conditional presence. A missing field
is not necessarily malformed if the event's rules make it conditional.
Likewise, a familiar field name does not imply that every event carries it.

## Distinguish enumerated values from an open vocabulary

A closed value list claims completeness under its definition. An open list can
grow, so consumers need a deliberate policy for values they do not recognize.

Some common fields belong to the record header rather than the payload. Their
documentation identifies that location. Do not assume that the same
information appears twice.

## Search the installed corpus

Exact lookup selects a named event or field. Prefix lookup helps explore a
subsystem, and keyword search matches names and summaries. Derived names can
resolve through documented suffix or standard-attribute rules; read which rule
supplied the result.

For example, look up an access-check event, a field, or a subsystem prefix:

```sh
evman kacs.audit.access.checked
evman subject.token.sid
evman kacs.audit
```

The field card lists events that carry the field. Use those relationships
when replacing a field-index lookup. The installed packages determine the
available definitions; a missing package can mean missing documentation.
For an older capture, keep the producer's version and catalog with the
record rather than assuming the current definitions are identical.

Packages supply definitions under `/usr/share/evman`. Catalog validation
checks consistency across those definitions. A valid catalog describes an
interface; it does not establish that a particular event was recorded or that
its payload passed runtime validation.

Use the event store or Event Viewer to investigate occurrences. Use `evman(1)`
for lookup and catalog-validation syntax.
