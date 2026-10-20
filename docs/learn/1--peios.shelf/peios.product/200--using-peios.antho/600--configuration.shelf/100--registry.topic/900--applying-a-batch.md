---
title: Applying a Registry Batch
type: how-to
description: Apply related registry changes atomically within one hive and handle uncertain commit outcomes.
---

Use a reviewed JSON registry document when several entries must become
visible together. A transaction provides registry atomicity, not simultaneous
application by every service that consumes the settings.

## Review, apply, and verify

1. Review every operation and its destination layer. All operations must
   target one hive, even when several hives share one source. Descriptor
   changes are security changes and require their own review and rights.
2. Save recovery data appropriate to the changes. A JSON export contains
   visible keys and effective values, not every descriptor or layer entry.
3. Apply the document:

   ```sh
   reg apply reviewed.json
   ```

4. Inspect the affected entries and verify consuming services. If a commit
   times out or loses its source reply, inspect state before resubmitting:
   storage can have committed despite the reported failure.

Text export is for review; text batch input is not implemented. `reg(1)`
defines the supported document schema.

## Boundaries of the transaction

`reg apply --dir DIR` applies each file as a separate transaction. A directory
import is not one atomic unit. Transactions have bounded time and size, no
nesting or savepoints, and require source support. Splitting a failed batch
can break the consistency requirement that motivated it.

Readers outside a transaction do not see its intermediate writes. Bound
reads see that transaction's pending changes. This does not provide
value-level conflict detection: if a write depends on an unchanged entry,
use `--expected-seq` with the destination layer's own sequence. `reg get -L`
only exposes that sequence when the destination is the effective winner.

A layer makes entries removable later; a transaction publishes related
changes together. Neither is an undo history, and removing a layer does not
reverse descriptor changes. A late successful commit can still notify
watchers after the caller's timeout. Closing an uncommitted transaction
aborts it, but cannot undo an already-dispatched successful commit.
