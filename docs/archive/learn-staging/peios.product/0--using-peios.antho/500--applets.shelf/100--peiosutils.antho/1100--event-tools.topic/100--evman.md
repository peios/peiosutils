---
title: evman
type: concept
description: evman documents what an event type or event field means — what it records, when it fires, what a value says — from shipped documentation, never the live event stream.
related:
  - peios/logs-and-events/overview

  - peios/registry-tools/regman
  - peios/documenting-events/writing-evman-pages
  - peios/pgss/events/the-catalogue
---

Every record in the event stream is a type name and a set of fields. The record says *that* something happened — `kacs.audit.access.checked`, with `outcome.success` false and an `access.granted` of `0x120089` — but not what any of it means. Whether `access.granted` is the rights the check granted or the rights a handle was opened with, whether a missing `trigger.ace` is a bug or expected, which of two masks you are looking at: that knowledge lives with the software that writes the records. It lives in **`evman`**, the event stream's manual.

`evman` is to events what [`regman`](~peios/registry-tools/regman) is to the registry. Give it an event type or a field and it tells you what that is. Reach for it whenever a record does not say enough on its own — before writing a query, a filter or a security descriptor over a field, and whenever a value surprises you.

## evman, in one sentence

**`evman <name>` documents what an event type or field *means* — when it fires, what each field records, which values it takes — drawn from shipped documentation, never from the live event stream.**

## Looking up an event type

Give `evman` an event type and it prints the event's card: how important it is, what decides whether it is recorded, a description, and every field it carries.

```
$ evman kacs.audit.access.checked

kacs.audit.access.checked                                      defined by kacs

  Tier         essential
  Gating       a matching SACL audit ACE, or the token's audit policy
  Cardinality  once per matching audit ACE, not once per access

The record that an access check completed, and what it decided. The most
common event on the system, and the one most investigations start from.
...

Fields
  subject                           group, from kernel
    subject.token.sid
    subject.token.groups
    ...
  object.kind                       required
  object.file.path                  when object.kind == file
  access.requested                  required
  access.granted                    required
      The mask this check granted.
  ...
```

| Fact | Tells you |
|---|---|
| **Tier** | How much the event matters, and whether an administrator can switch it off: `essential` events are always recorded; `standard` ones are on by default; `verbose` and `debug` ones are off unless switched on. |
| **Gating** | What decides which occurrences produce a record at all. An event gated on a SACL is silent for an object with no audit ACE, so its absence proves nothing. |
| **Cardinality** | How many records one occurrence produces, where that is not one. |

Each field line says when the field is present — always, optionally, or only when another field has a given value — and an indented note says what the field means *in this event*, where that is narrower than its general meaning. Read those notes: the same field can mean subtly different things in different events, and the note is where that is written down.

## Looking up a field

Give `evman` a field and it prints the field's card: its type, the values it can take, a description, and every event type that carries it.

```
$ evman subject.token.type

subject.token.type                                           defined by kernel

  Type    str.enum
  Values  primary | impersonation
  Closed  true

Whether the effective token is a primary token or an impersonation token.
Needed beside subject.token.impersonation, because a primary token and an
Anonymous impersonation token both report level 0 there.

Carried by
  kacs.audit.access.checked  (via group subject)
  kacs.audit.handle.used     (via group subject)
  lcs.audit.key.opened       (via group caller)
  ...
```

**Carried by** is the reverse index: the event types whose records can hold this field. It is the list to check before you rely on a field across the whole stream — a query on `subject.token.sid` finds nothing in an event type that does not carry it.

**Closed** says whether the list of values is complete. A closed list never grows without the field changing; an open one may gain values, so anything you write over it should cope with a value it has not seen.

Some fields are carried in the record header rather than the payload — the time, the sequence number and who wrote the record. Their cards say so: they are on every event, and no payload repeats them.

## Looking up a subtree

Field names and event types are both dotted, general to specific. Give `evman` a prefix and it lists what lies beneath it, one summary line each:

```
$ evman kacs.audit

Event types
  kacs.audit.access.checked  The record that an access check completed, and…
  kacs.audit.handle.used     The record of what was done with a handle afte…
  kacs.audit.privilege.used  The record that a privilege contributed access…
```

```
$ evman subject.token

Fields
  subject.token.auth-id             The LUID of the logon session the effec…
  subject.token.groups              The group SIDs carried by the effective…
  subject.token.integrity           The integrity RID of the effective toke…
  ...
```

So you can start from a subsystem, or from a part of a record such as "who acted", and drill into any one entry.

A few names are not defined one by one. Any field can have a variant with a suffix such as `-previous` or `-requested` — `policy.generation-previous` is the generation that was in force before — and any thing can have a standard attribute such as `count` or `size`. `evman` resolves these to the field or thing they qualify and says which rule it applied.

## Searching, when you do not know the name

`evman -k` searches names and summaries — the event stream's `apropos`. Every term must match.

```
$ evman -k swap
buffer.capacity-previous    The per-CPU ring capacity before a capacity swap…
buffer.generation-previous  The ring generation before a capacity swap, nor…
kmes.buffer.swap.failed     The record that KMES could not resize its per-C…
```

## evman documents meaning, not what happened

**`evman` tells you what an event or field *means* — never which events have occurred on this machine.** It reads shipped documentation and never touches the event stream or the event store. Ask it about `kacs.audit.access.checked` and you learn when the event fires and what its fields hold; you do not learn that anything was denied today. That is a query against the event store, which is the [Event Viewer](https://learn.peios.org/peios/desktop-event-viewer/event-viewer/)'s job, or eventd's query interface.

| To learn… | Look at… |
|---|---|
| What an event or field *means* | `evman` — the manual |
| Which events have *happened* | the [Event Viewer](https://learn.peios.org/peios/desktop-event-viewer/event-viewer/), or a query against eventd |
| Whether an event type is *recorded* at all | its tier, and the emission policy under `Machine\Generic\Events` — see [`regman`](~peios/registry-tools/regman) |

## Where the documentation comes from

`evman` has no built-in list of events. Each package ships documentation for the events it writes and the fields it defines, as files in `/usr/share/evman/`. Install a package and its events appear in the manual; remove it and they go.

The manual is also the definition. Every field any Peios component writes is defined in exactly one of these files, by the component that owns it, and a field defined nowhere is not a field anything should emit. That is what keeps one value from acquiring two names: a security descriptor that denies a field must not be evadable by writing the same value under another name. The rules are in [The Catalogue](~peios/pgss/events/the-catalogue).

(For the people who *write* these files, `evman lint` checks them against those rules. That is a packaging concern rather than an everyday one; see [Writing evman pages](~peios/documenting-events/writing-evman-pages).)

## Where to go next

- [Logs and events](~peios/logs-and-events/overview) — what the event stream is, and how events differ from logs.
- [`regman`](~peios/registry-tools/regman) — the same idea for the registry, including the emission-policy keys that switch event types on and off.
