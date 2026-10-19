---
title: regman
type: concept
description: regman documents what a registry key or value means — type, default, valid range, and when a change applies — from shipped docs, never the live registry.
related:
  - peios/registry-administration/reg
  - peios/registry-concepts/configuration-and-meaning
  - peios/registry-concepts/keys-values-and-types
  - peios/registry-administration/lcs-and-sources
  - peios/auditing/overview
  - peios/registry-concepts/overview
---

[Configuration, not storage](~peios/registry-concepts/configuration-and-meaning) established that the registry holds values but not their meaning: it keeps a type tag and some bytes, and never knows that `BufferCapacity` must be a power of two or what it is for. That knowledge has to live *somewhere* a person can look it up. It lives in **`regman`**, the registry's manual.

`regman` is the `man` of the registry. Give it a path and it tells you what a key or value actually is. It is the everyday tool for configuring a Peios system — before you touch a knob, `regman` is how you find out what it does and what changing it will cost.

## regman, in one sentence

**`regman <path> [value]` documents what a registry key or value *means* — its type, default, valid values, when a change takes effect, and a prose description — drawn from shipped documentation, never from the live registry.**

That last clause is the one to hold onto; there is a section on it below.

## Looking up a value

Give `regman` a key path and a value name and it prints a **knob-card** — an identity line, an aligned block of facts, and a description:

```
$ regman 'Machine\System\KMES' BufferCapacity

Machine\System\KMES BufferCapacity                    documented by kmes

  Type     REG_QWORD
  Default  4194304  (4 MB)
  Valid    65536–268435456 bytes (64 KB–256 MB), power of two
  Applies  live — ring-buffer swap

Per-CPU ring buffer capacity, in bytes. Must be a power of two; values
that are not are treated as invalid and ignored.
```

Every registry value is a *knob*, and every knob has the same few facts worth knowing, so the card is uniform rather than free-form:

| Field | Tells you |
|---|---|
| **Type** | The value's registry type (`REG_QWORD`, `REG_SZ`, …). |
| **Default** | The value used when nothing is set. |
| **Valid** | The range, set, or constraint a sensible value must satisfy. |
| **Applies** | When a change takes effect — `live`, on `restart`, or on `reboot`. |

The **Applies** field is the one operators reach for most: it is the difference between "edit it and you're done" and "edit it and schedule a reboot". Only the fields that make sense for a given item are shown, and a knob that is being retired carries a prominent deprecation notice at the top of its card.

## Looking up a key

Give `regman` just a key — no value — and it does double duty as an *index*: the key's own description, then a list of every value documented under it, one summary line each.

```
$ regman 'Machine\System\KMES'

Machine\System\KMES                                   documented by kmes

The KMES event subsystem reads its tuning parameters from the values
under this key...

Values
  BufferCapacity         Per-CPU ring buffer capacity in bytes.
  MaxEventSize           Max total size of a userspace-emitted event.
  MaxNestingDepth        Max nesting depth for userspace event payloads.
  MaxEmitRatePerProcess  Max events per second a single process may emit.
```

So you can start at a subtree, see what is configurable there, and drill into any single value.

## Keys that come in families

Much of the registry is not at a fixed path. Every service has a key under `Machine\System\Services\`, every interface one under `Machine\System\Network\Interfaces\`, every principal one under `Machine\Generic\Authn\Policy\`. You ask about a real one — `sshd`, an interface id — and the documentation is written once for the whole family.

`regman` resolves that for you. Ask about the concrete path and you get the family's page:

```
$ regman 'Machine\System\Services\sshd' ImagePath

Machine\System\Services\<name> ImagePath              documented by peinit

  Type     REG_SZ
  Default  (required)
  Valid    a non-empty absolute path
  Applies  restart

Absolute path to the service binary.
```

The heading keeps the `<name>` so you can see you have been handed the page for the family rather than one written about `sshd` specifically. Everything on the card applies to the key you asked about.

A heading with a trailing `...` — `Machine\System\Network\Rules\<Layer>\<rule...>` — means the family is a *tree* and the page covers it at any depth. That is what firewall rules are: an exception is a subkey of a rule, and a rule nested five deep has the same values meaning the same things as one at the top, so one page answers for all of them.

If a package *has* documented your exact key — its own service's knob, a particular principal's privileges — that page wins instead, and the heading shows the concrete path. The more specific documentation always answers.

## Searching, when you do not know the path

If you don't know where a setting lives, `regman -k` searches names and summaries — the registry's `apropos`:

```
$ regman -k buffer
Machine\System\KMES BufferCapacity   Per-CPU ring buffer capacity in bytes.
```

## regman documents intent, not current state

This is the boundary that matters most. **`regman` tells you what a setting *should be* — what it means and which values are legal — not what it is currently set to on this machine.** It reads shipped documentation, never the live registry; it does not talk to [LCS](~peios/registry-administration/lcs-and-sources) at all. Ask `regman` about `BufferCapacity` and you learn it must be a power of two and defaults to 4 MB; you do *not* learn that this particular box has it set to 8 MB right now. Reading the current value is a separate, live query against the registry — that is [`reg`](~peios/registry-administration/reg)'s job. `regman` tells you a knob *should* be a power of two; `reg get` tells you what it is set to, and `reg set` changes it. Reach for `regman` to decide what to write, and `reg` to write it.

Put `regman` next to the other two things from [Configuration, not storage](~peios/registry-concepts/configuration-and-meaning) and a clean division of labour appears:

| To learn… | Look at… |
|---|---|
| What a setting *means*, and what is valid | `regman` — the manual |
| What the value is *set to* | [`reg`](~peios/registry-administration/reg) — a live query against the store |
| What the subsystem is *actually running on* | the [event log](~peios/auditing/overview) |

`regman` is the one you consult first, and the only one that works before you have changed anything — because documentation ships with the software, not with the machine's state. It is the natural complement to [reject-or-keep](~peios/registry-concepts/configuration-and-meaning): `regman` tells you the valid range up front, so you set a good value the first time instead of writing one the owning subsystem will quietly refuse.

## Where the documentation comes from

`regman` has no built-in database of every setting. Each package **ships its own documentation** as files dropped into a well-known directory (`/usr/share/regman/`), one fragment per package documenting that package's whole configuration surface. Install a package and its registry documentation appears; remove the package and it goes away. The package that *owns* a set of keys is the package that documents them — the only arrangement that stays correct as the system changes.

A consequence worth knowing: `regman` can only describe settings that some installed package has documented. An undocumented key is invisible to it — `regman` is exactly as complete as the packages on the machine make it. If two packages happen to document the same item, `regman` shows both and flags the overlap rather than silently picking a winner.

(For the people who *write* that documentation, `regman fmt` and `regman lint` prepare and check fragments, and `regman index` keeps lookups fast on large corpora — the lookup is always correct without an index, which is purely an accelerator. These are packaging concerns rather than everyday operator ones.)

## Where to go next

For the idea `regman` exists to serve — why the registry holds values without holding their meaning, and what reject-or-keep means — read [Configuration, not storage](~peios/registry-concepts/configuration-and-meaning).

For the other half of the picture — reading what a value is actually *set to*, which is a live query against the store rather than a documentation lookup — read [LCS and sources](~peios/registry-administration/lcs-and-sources).
