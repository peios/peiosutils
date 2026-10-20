---
title: Removing Registry Entries
type: how-to
description: Choose withdrawal, masking, or hiding and verify which value or key becomes visible.
---

Decide whether the intended result is to withdraw one layer's contribution
or make a name absent despite contributions beneath it. These are different
operations.

## Choose the operation

| Intended result | Operation |
|---|---|
| Withdraw a layer's value entry | `reg delvalue` |
| Keep a value absent over lower entries | `reg mask` |
| Withdraw a layer's key-existence claim | `reg delkey` |
| Hide a key over lower claims | `reg hidekey` |
| Remove the corresponding absence marker | `reg unmask` or `reg unhidekey` |

`reg mask --all` creates a blanket value mask. It does not delete every
stored value. Use `reg(1)` for exact arguments and layer selection.

1. Inspect the target and winning layer, then save the recovery data needed
   for that scope.
2. Select the destination layer explicitly. Do not assume withdrawing its
   entry makes the name absent: another layer can become visible.
3. Run the operation appropriate to the intended result.
4. Reopen the path, inspect the effective state, and verify the consumer.
   Removing an absence marker can reveal data that has changed since masking.

Key deletion requires key deletion rights; value changes need value-write
rights. Both also require applicable layer access. Hive roots cannot be
hidden or deleted through these operations.

## Recursive removal and existing handles

`reg del -r` uses one transaction and does not follow registry links during
its recursive walk. Resource and transaction limits still apply; a large
walk can fail. Terminal confirmation is not shown with redirected input.

A lost commit reply leaves the outcome uncertain. Read state before retrying
rather than assuming nothing changed. Existing handles can continue to
refer to an orphaned key after its path disappears. A replacement at that
path is a different object; reopening is necessary to inspect or watch it.
