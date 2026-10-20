---
title: Recovering a Registry Layer
type: how-to
description: Inspect the effective winner and withdraw a configuration layer without mistaking it for a full rollback.
---

A layer keeps a bundle's entries identifiable and removable. It is not a
snapshot of everything the bundle changed. Prefer the owning role or policy
tool when it also manages files, service actions, or other state.

## Inspect and recover

1. Record the effective value, its winning layer, and its sequence with
   `reg get KEY NAME -L`. Inspect the layer inventory with `reg layer ls -l`.
2. Identify the layer's owner and recovery procedure. Save required data and
   layer metadata before changing it. A narrow subtree backup need not
   include the definitions that make its layer entries active.
3. If the owner's workflow permits it, disable the named layer first:

   ```sh
   reg layer set NAME --disable
   ```

4. Reread representative values, check the newly winning layer, and verify
   each affected consumer at its documented application point. Re-enable
   with `--enable` if the trial does not produce the intended result.
5. Delete a layer only after its owner confirms it is unneeded and the
replacement
   state is verified. `reg layer del NAME` is destructive. Its terminal
   confirmation is absent with redirected input.

Replace `KEY`, `NAME`, and the value name with the reviewed targets. The
built-in base layer cannot be disabled or deleted.

## Limits of layer recovery

Disabling a layer removes it from ordinary global resolution. Credentials
that explicitly attach the layer can still see it. Inspect the workload's
effective thread credentials before concluding that it sees the same state
as an administrator's shell.

Removing a layer does not restore key security descriptors, reverse actions
already taken by services, or reconstruct earlier values overwritten within
that layer. Consumers do not necessarily apply the new winner immediately.

An ordinary write into a new layer is different from retaining previous
versions. Equal-precedence layers use write sequence as their tiebreaker,
not a fixed order of role names. See [Registry Reads, Writes, and
Layers](~peios/registry/reg) for effective reads and authorization.
