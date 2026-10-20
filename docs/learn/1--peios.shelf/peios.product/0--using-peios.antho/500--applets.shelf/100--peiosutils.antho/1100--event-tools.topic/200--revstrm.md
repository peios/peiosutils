---
title: Inspecting the Raw Event Stream
type: how-to
description: Use revstrm for transport diagnosis while accounting for bounded rings, display filters, and incomplete output.
---

`revstrm` reads KMES directly, without eventd's storage or query path. It is a
diagnostic consumer, not a durable audit sink. Use eventd for stored history
and retain its collection/retention limits when drawing conclusions.

## Observe a bounded stream

1. Confirm that the caller has enabled `SeSecurityPrivilege` and the native
   event interface is available. Administrator membership alone does not
   establish that the privilege is enabled.
2. Run `revstrm` to follow, or `revstrm --snapshot` to drain available data
   and exit. Snapshot drains CPU rings in order; it is not an atomic
   cross-CPU snapshot or a request for history predating attachment.
3. Narrow display with repeatable `--type` globs or `--origin` classes. Use
   `--pretty` to expand payloads when studying individual records.
4. Record loss markers and stderr with the output. A ring that falls behind
   overwrites old data, and display filtering does not prevent that loss.

The header gives UTC time of day, CPU, per-ring sequence, origin, and event
type. It omits the calendar date and does not merge CPUs into one event order.
Do not interpret a displayed sequence gap without accounting for filters.

## Interpret payloads and completion

The installed event catalog supplies field types. Known SIDs, GUIDs, masks,
enumerations, timestamps, durations, and byte counts receive readable forms;
unknown bits and values remain numeric. Without a matching catalog entry,
field-name fallbacks are less authoritative. Malformed MessagePack falls
back to a hex preview rather than becoming a decoded record.

Compact output can truncate a long payload. Pretty output expands it but
can slow draining. Redirecting to a file or reducing output cost can help;
neither guarantees lossless capture. Native file policy applies to the
chosen destination.

Follow mode ends on interruption, output failure, or exhausted drain threads.
The implementation treats output failure as termination and can finish after
per-ring errors, so a zero exit status alone is not a complete-capture
attestation. Inspect diagnostics and loss accounting. `revstrm(1)` defines
its option and output details; `evman` explains event schemas.
