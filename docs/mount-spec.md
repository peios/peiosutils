# peiosutils `mount` / `umount` — Implementation Specification

Status: **CONVERGED draft.** Version **1.9** (4 Opus + 3 Sonnet + final Opus audit rounds; final round clean on 3 axes, one internal contradiction fixed — see §0).

This spec defines the v1 behaviour of the `mount` and `umount` applets in peiosutils (a
uutils/coreutils-derived multicall binary).

The design intent is **faithful reproduction of util-linux `mount(8)`/`umount(8)`
behaviour**, restricted to the surface defined in §2, and adapted to the peios kernel
(pkm, Linux 7.0.9 base) and its KACS security model. Anything in util-linux mount that is
**not** listed as deferred in §3 and **not** specified as kept here is a spec gap to be
flagged. Audit instructions are in §15.

---

## 0. Revision history

**v1.9 — final Opus round (flags / semantics / umount+listing all CLEAN & converged; adversarial found one internal contradiction):**
- §4: **propagation carved out of the mutual-exclusivity rule.** Previously §4.6 listed
  `propagation` among the mutually-exclusive verbs, contradicting §17 (which specifies
  `mount <verb> … --make-*` as a verb + trailing propagation step). util-linux (≥2.23) allows
  combining propagation with another operation, and allows multiple propagation changes in one
  command. New §4.6 = structural verbs (bind/rbind/move/beneath/remount) mutually exclusive;
  new §4.7 = propagation may stand alone or accompany another verb/new-mount as trailing
  `mount_setattr` step(s), and multiples are allowed. (The v1.8 §6.4 `-o`-token classification
  had sharpened this latent collision.)
- All v1.8 edits re-verified factually correct (incl. `optmap.c` confirmation that the `-o`
  propagation tokens are `MNT_NOMTAB`, i.e. command-line-valid, not fstab-only).

**v1.8 — Sonnet v1.7-confirm round (flags + semantics clean; propagation rewrite source-verified correct; 3 small items):**
- §6.4: classified the **propagation `-o` tokens** (`shared`/`slave`/`private`/`unbindable` + recursive `rshared`/`rslave`/`rprivate`/`runbindable`) as Category-D meta-verbs routing to the propagation op (§2.1) — they were referenced in §2.1 but unclassified in the `-o` partitioning. (util-linux accepts these as `-o` options per `mount(8)`.)
- §11: broadened the source-resolution wording — `mnt_pretty_path` runs full `realpath()` on any symlink (not just `/dev/disk/by-*`) and maps `/dev/loopN`→backing-file, `/dev/dm-N`→`/dev/mapper/<name>`.
- §2.1: corrected the parenthetical — util-linux uses `mount_setattr` for *recursive* `--make-*` since v2.39 (our `mount_setattr`-for-all choice converges with upstream).

**v1.7 — Sonnet confirming round (caught a core-mechanism factual error 8 prior auditor-instances accepted):**
- **§1.3/§2.1: propagation-type changes use `mount_setattr`'s `propagation` field (Linux 5.12+), NOT classic `mount(2)`.** The prior "no new-API verb for propagation" claim was *false* — `mount_setattr` is that verb (and gives atomic `AT_RECURSIVE` propagation for `--make-r*`). Corrected the mechanism and the §1.3 rationale; `MOVE_MOUNT_SET_GROUP` note retained (it is peer-group membership, a different op). §17 partial-failure note updated accordingly.
- §6.7: generalized the bind-remount guard — **any** superblock-level option (Category B/C + `=fs`), not just `ro=fs`/`rw=fs`, is a usage error on a bind remount (userspace policy guard).
- §12: the fs_context kernel log is drained+reported **on failure regardless of `-v`** (it's the error reason, not just verbose detail).
- §10: corrected the exit-64 claim — 64 *can* arise from additive `rc` across multiple source args (faithful); a single `-A`/`-R`/`-A -R` invocation is stop-on-first-failure → 32.
- §11: corrected the source-tag wording (no tag→device evaluation at list time; mountinfo already stores the device path; `mnt_pretty_path` only resolves `/dev/disk/by-*` symlinks).
- §6.7: strengthened the recursive-`mount_setattr` all-or-nothing note (kernel-source-verified `prepare`/`commit`, NOT man-page-documented → provium-confirm); §17 SIGINT wording tightened; §2.6 `-r`/`-R` exclusion cited to `umount.c excl[]`.

**v1.6 — Sonnet 4.6 cross-model sweep (4 auditors + a fact-check; caught real kernel-mechanism items the 4 Opus rounds shared a blind spot on):**
- §6.9: corrected `X-mount.nocanonicalize=type` values to **`source`/`target`** (were wrongly `all`/`mtab`/`fstab`) — fact-verified against `mount(8)`.
- §2.1: `--exclusive` pinned to **`FSCONFIG_CMD_CREATE_EXCL`** (Linux 6.6+, `EBUSY` if it'd reuse); added the two `move_mount(2)` **`--beneath` propagation constraints** (parent→top-mount, parent→source); added a note that `MOVE_MOUNT_SET_GROUP` is peer-group membership, not a propagation-type verb (so classic `mount(2)` for `--make-*` is correct); fixed the fresh-mount propagation-default wording (removed the self-contradiction; marked provium-verify).
- §11/§3: **reverted the v1.5 `-O`-listing-filter overreach** — `-O` does NOT filter the listing (util-linux ignores it without `-a`); only `-t` does.
- §2.5/§3: added **mount-side `-n/--no-mtab`** (accepted+ignored), symmetric with umount.
- §2.6: `umount -R` and `-r` are **mutually exclusive** (exit 1).
- §3: classified **`ID=` source tag** as cut (udev/WWN dependency, "not recommended" upstream).
- §6.7: relabelled `ro=fs`-on-bind-remount as a **userspace policy guard** (the kernel would NOT refuse it); added an `FSCONFIG_CMD_RECONFIGURE` per-option-atomicity caveat.
- §12: `-v` log-drain **buffer policy** (avoid `EMSGSIZE`-consumes-and-loses kernel error messages).
- §17: combined verb + propagation **partial-failure** behaviour (leave the mount, report the propagation failure).
- §5.1: documented post-`LOOP_CLR_FD` **lazy-destruction** limitation.
- **Confirmed CORRECT (no change, source-verified by the fact-check):** `mount_setattr`+`AT_RECURSIVE` all-or-nothing (kernel `prepare`/`commit`); `umount -A` → 32-never-64 (`umount_alltargets` stops on first failure). These resolved two inter-auditor disputes in the spec's favour.

**v1.5 — audit pass 4 (4× Opus 4.8; 3 of 4 axes fully clean, 1 classification gap + 2 cosmetic):**
- §11/§3: classified the no-operand **listing-filter** meaning of `-t`/`-O` (incl. type-lists/
  `no<type>`) as **kept** — filtering a materialized list is trivial and faithful; distinct
  from the mounting-path type-list deferral.
- §2.5: added the `--ro` long-alias (symmetry with `--rw`).
- §10: noted exit **64** is not produced for the kept multi-target umount ops (`-A`/`-R`/`-A -R`
  use stop-on-first-failure → 32), symmetric to the 16/64 deferred-feature line.

**v1.4 — audit pass 3 (4× Opus 4.8; auditor 1 fully clean, §6.7 kernel claims source-verified correct):**
- §6.7: specified `ro=fs`/`rw=fs` on remount route through `fspick`+`fsconfig` (superblock
  scope), not per-mount `mount_setattr`; and `ro=fs`/`rw=fs` on a *bind* remount is
  unsatisfiable → honest usage error (exit 1), never a silent drop or shared-sb mutation.
- §6.7: specified recursive-remount is **all-or-nothing** (kernel validates the whole subtree
  before committing); partial-failure surfaces exit 32 with no partial application.
- §2.6: specified the `umount -A -R` composition (recurse under each of the source's mountpoints; stop on first failure).
- §10: enumerated mount-side exit **126** (helper exec failure) for symmetry with `umount`.

**v1.3 — recursive / remount review:**
- §6.7 rewritten: the "read mountinfo, re-supply unspecified flags" preserve-dance is an
  old-`mount(2)`-`MS_REMOUNT` reset artifact and is **unnecessary on the new-API path**
  (`mount_setattr` masks; `fsconfig` reconfigure applies deltas — neither resets). Retained
  only for the §1.3 classic fallback. Generalizes §1.5: don't port util-linux workarounds for
  `mount(2)` limitations we don't have.
- **`=recursive` on remount is now supported** (`mount_setattr`+`AT_RECURSIVE`; reversed the
  v1.2 rejection — the kernel mechanism is solid on 7.0.9, and "EXPERIMENTAL" upstream is a
  CLI-stability label, not a mechanism-reliability one).
- `X-mount.subdir` disposition settled on evidence (man page + libmount `is_subdir_required()`):
  it is **effective only on a new-instance mount and silently ignored for bind/rbind/move/
  remount/propagation** (reversed the v1.2 "reject the combination" — upstream silently drops
  it, doesn't error). Removes the §6.9 composition ambiguity (`subdir` and `rbind` are
  mutually-exclusive contexts).

**v1.2 — audit pass 2 (4× Opus 4.8 against `mount(8)`/`umount(8)` + util-linux source):**
- §6.1: removed invented `symfollow` token (util-linux has only `nosymfollow`).
- §5: added loop-device **re-use** (same backing file + offset + sizelimit → reuse, not re-allocate — corruption avoidance, util-linux 2.29+); narrowed the implicit-loop trigger (type unspecified OR fs known to libblkid); reconciled `LO_FLAGS_AUTOCLEAR` with `umount -d` (§5.1).
- §6.7: bind-mount remount is **per-mount-only** (`mount_setattr`, never sb `fsconfig`); per-layer preservation defined; `=recursive` on remount rejected in v1.
- §6.8/§6.9/§2.1: defined `open_tree` flag composition; **rejected** `X-mount.subdir` + `rbind`/recursive combinations in v1 (EXPERIMENTAL upstream).
- §2.1: added `--beneath` kernel refusal constraints, move-into-shared `EINVAL`, `--exclusive` vs `--onlyonce` error contract, and fresh-mount **propagation defaults** (private under the new API).
- §2.5: corrected `--onlyonce` (driver-dependent, fs-root aware); added `-w`/`rw` **auto-ro-fallback suppression** semantics; `-vv` ≈ `-v`.
- §2.6: kept **`umount -A/--all-targets`** (live-mountinfo, not fstab — moved out of §3); added `-R` over-mount semantics; documented `-c`/`-g` non-root divergences.
- §10: enumerated `umount` exit **126** (helper exec failure) for the kept helper path.
- §11: corrected "canonical order" → positional **VFS→FS→user merge with ro/rw coalescing** (no sorting); added unmangle/escape decoding, control-char→`?`, source-tag evaluation, torn-read tolerance, and the parent-filtered information-leak rule.
- New §1.8 (byte/`OsStr` path & option handling) and §17 (robustness: signal-safe cleanup, mount-side no-follow/TOCTOU, option-string length).
- §3: added swap (`swapon`/`swapoff` domain) to the cut list.

**v1.1 — audit pass 1 (4× Opus 4.8):** corrected `defaults` (dropped erroneous `relatime`);
reframed single-positional as ambiguous; added remount flag-preservation, loop autoclear +
EBUSY-retry, bind-attr (`-o bind,ro`), `--beneath`/`--exclusive`/`-m`/`ro=/rw=` suffix,
umount `-d/-r/-g/-q/--fake/-i` + `umount2` table; split functional `X-mount.*` from fstab
comment fields; cut idmapped mounts and POSIX `X-mount.owner/group/mode`; added library
architecture (§16).

---

## 1. Scope & guiding principles

1.1. **Faithful to util-linux** for everything in the kept surface: same option spellings,
semantics, argument-shape disambiguation, exit codes, and listing format.

1.2. **No fstab, no mtab.** peios has no `/etc/fstab` and no `/etc/mtab`. The future
"registry" fstab-equivalent is **Ring 3**, out of scope (§3). Live mount state is read from
the kernel (`/proc/self/mountinfo`).

1.3. **New fd-based mount API is primary** (`fsopen`/`fsconfig`/`fsmount`/`move_mount`/
`fspick`/`open_tree`/`mount_setattr`) — **including propagation-type changes, which use
`mount_setattr`'s `propagation` field (Linux 5.12+), NOT classic `mount(2)`** (see §2.1).
`umount2(2)` is used for unmounting (the new API has no unmount verb). pkm 7.0.9 has 100%
native `init_fs_context` coverage, so `fsconfig` per-option errors work everywhere; even an
unconverted fs mounts through the kernel `legacy_fs_context` shim. A classic-`mount(2)`
instantiation fallback is added only if a concrete fs fails through `fsopen` (none known).

1.4. **Privilege model: no coarse identity checks.** Never a `getuid()==0` test; KACS
authorises per-operation. **The applets are not installed setuid** — so util-linux's
"drop suid and continue unprivileged" behaviour (≥2.35) does not apply; authorization is
entirely KACS per-op. Fine-grained capability prechecks are permitted only where they
improve atomicity/ergonomics; none are required in v1. Where util-linux silently suppresses
an option for non-root (e.g. `-c` on umount, `-g`'s root-only effectiveness), peios applies
it **unconditionally** — a deliberate divergence consistent with this model (§2.6 notes).

1.5. **The tool is never the source of weirdness.** It always does the correct, honest
thing. Clumsy outcomes caused by immature kernel subsystems (e.g. mount-namespace authz)
are surfaced honestly, not papered over and not caused by the tool.

1.6. **License:** peiosutils stays **MIT**. libblkid (§7) is a separate **LGPL-2.1-or-later**
package, dynamically linked. SDDL handling reuses the `peios` crate's `security::sddl`.

1.7. **Implementation structure:** the mount logic lives in a **library crate**; the CLI is
a thin wrapper (§16).

1.8. **Byte-accurate path/option handling.** Paths, `-o key=value` values, labels/UUIDs, and
SDDL are handled as **opaque byte strings** (`OsStr`/`&[u8]`), never assumed UTF-8. Embedded
NUL is rejected with a usage error; non-UTF-8 bytes pass through losslessly to
`fsconfig`/`move_mount`/libblkid. The `key=value` first-`=` split (§2.4) and SDDL parsing
(§8.4) operate at the byte level.

---

## 2. Kept surface (v1)

### 2.1 Operation modes (verbs)

| Verb | Trigger | New-API mechanism |
|------|---------|-------------------|
| **New mount** | `mount [-t T] SRC TGT` | `fsopen`→`fsconfig`→`fsmount`→`move_mount` |
| **Bind** | `--bind\|-B` or `-o bind` | `open_tree(OPEN_TREE_CLONE)`→(attrs §6.8)→`move_mount` |
| **Recursive bind** | `--rbind\|-R` or `-o rbind` | `open_tree(OPEN_TREE_CLONE\|AT_RECURSIVE)`→`move_mount` |
| **Move** | `--move\|-M` or `-o move` | `move_mount(MOVE_MOUNT_*)` |
| **Move beneath** | `--beneath SRC TGT` | `move_mount(MOVE_MOUNT_BENEATH)` (constraints below) |
| **Remount** | `-o remount[,…] TGT` | flag-preserving reconfig (§6.7) |
| **Propagation** | `--make-{shared,slave,private,unbindable}` (+ `r`-recursive) or `-o` forms | `mount_setattr` with the `propagation` field set to `MS_SHARED`/`MS_SLAVE`/`MS_PRIVATE`/`MS_UNBINDABLE` (`+AT_RECURSIVE` for the `r`-variants) |
| **List** | `mount` / `mount -l` | read `/proc/self/mountinfo` (+ libblkid for `-l`) |

Propagation-type changes use **`mount_setattr`'s `propagation` field** (Linux 5.12+) — this is
the new-API verb (consistent with §1.3), and for the recursive `--make-r*` variants it carries
the same all-or-nothing `AT_RECURSIVE` `prepare`/`commit` guarantee as recursive remount (§6.7).
(util-linux uses classic `mount(2)` `MS_*` for *non-recursive* `--make-*` but `mount_setattr`
for the *recursive* variants since v2.39 — so our `mount_setattr`-for-all choice converges with
upstream; classic `mount(2)` remains a valid fallback.)
`move_mount(MOVE_MOUNT_SET_GROUP)` is a *different* operation — it adds a mount into an existing
peer *group* (sharing-group membership), not a propagation-type change — so it is not used here.

**Fresh-mount propagation default:** a new mount / bind / move created via the fd-based API is
attached **private** by default (the kernel's new-mount propagation rule: a new mount is private
unless its destination's parent has shared propagation, in which case it joins that peer group).
This matches classic `mount(2)` propagation-inheritance behaviour. (The exact propagation a
detached `fsmount`/`open_tree(OPEN_TREE_CLONE)` tree carries until `move_mount` is **verify at
implementation** — the public man pages do not pin it; confirm with a provium test.) Adjustable
with `--make-*`.

**`--beneath` refusal constraints (kernel; surface honestly as exit 32 / `EINVAL`):** cannot
attach beneath a filesystem root (incl. chroot/pivot_root roots); target must not be a
detached mount; top mount + its parent must be in the caller's mount ns; the caller must be
able to unmount the current top mount; the target's mount must not be an ancestor of the
source; the source must have no overmounts; **the parent of the current top mount must not
propagate to the top mount**; and **the parent of the current top mount must not propagate to
the source being mounted beneath**. (The last two are `move_mount(2)` propagation constraints.)

**Move into a shared subtree:** if the source mount's parent has shared propagation, the
kernel refuses the move (`EINVAL`) — surfaced as exit 32.

**`--exclusive`** forces a unique superblock instance (no sb reuse) on the **new mount**
verb's `fsopen` path — concretely via **`FSCONFIG_CMD_CREATE_EXCL`** (Linux 6.6+) rather than
`FSCONFIG_CMD_CREATE` (which silently reuses an extant instance); the kernel returns `EBUSY`
if it would be forced to reuse. util-linux allows it for unprivileged users (peios: KACS per-op).
Distinct from `--onlyonce` (which dedups by source+target, §2.5). Because a block device
yields one shared superblock and the kernel refuses a second *writable* instance (§8.2),
`--exclusive` on an already-mounted writable block device fails (`EBUSY` → exit 32); it is
meaningful only for multi-instance filesystems (tmpfs) or read-only block mounts.

**Over-mounting:** mounting onto a target that already has a mount, or a non-empty/busy
directory, is **permitted and silent** (prior contents are hidden), matching util-linux;
`--onlyonce` is the opt-out.

### 2.2 Source specification

| Form | Resolution |
|------|-----------|
| Device path (`/dev/sda1`) | used directly |
| `-L`/`LABEL=`, `-U`/`UUID=`, `PARTLABEL=`/`PARTUUID=` | libblkid → device |
| directory / regular file (bind source/target) | used directly (a file may be a bind target) |
| pseudo / `none` (tmpfs, proc, sysfs, …) | passed through |
| image file via loop | §5 |

### 2.3 Filesystem type

- `-t TYPE` — explicit.
- `-t auto` / **omitted** — libblkid probes (§7); implied for any non-pseudo source when `-t`
  is absent. (util-linux's secondary `/proc/filesystems` list-probing fallback is dropped as
  part of the fstab/type-list deferral, §3.)
- Type **lists** and **`no<type>` negation** are **deferred** (batch-only; §3).

### 2.4 `-o` option language

Comma-separated `key`/`key=value` tokens; `key="..."` quoting protects embedded commas;
`key=value` splits on the **first** `=` (byte-level, §1.8). Partitioned (§6) into: (A)
per-mount attrs, (B) superblock flags, (C) fs-specific passthrough, (D) userspace-only, (E)
KACS policy, (F) functional `X-mount.*`.

### 2.5 Command-line flags (kept)

| Flag | Meaning |
|------|---------|
| `-t, --types TYPE` | filesystem type (incl. `auto`) |
| `-o, --options LIST` | option list (§2.4) |
| `-r, --ro, --read-only` | `-o ro` |
| `-w, --rw, --read-write` | `-o rw`; **and disables the auto-ro-fallback** (§2.5.1) |
| `--source` / `--target` | explicit operands |
| `--target-prefix DIR` | prepend `DIR` to the target path |
| `-B/--bind`, `-R/--rbind`, `-M/--move`, `--beneath` | verb selectors (§2.1) |
| `--make-*` (+ `--make-r*`) | propagation (§2.1) |
| `--exclusive` | unique superblock instance (§2.1) |
| `-m, --mkdir[=mode]` | create target dir if missing (default 0755); alias of `-o X-mount.mkdir` |
| `-c, --no-canonicalize` | do not canonicalize paths (applied unconditionally, §1.4) |
| `-f, --fake` | dry run (§12) |
| `-v, --verbose` | verbose + drain new-API kernel log (§12); repeated `-vv` ≈ `-v` |
| `-l, --show-labels` | augment listing with fs labels |
| `--onlyonce` | skip if already mounted (§2.5.2) |
| `-N, --namespace NS` | operate inside mount namespace `NS` (§13) |
| `-i, --internal-only` | do not invoke a `mount.<type>` helper (§9) |
| `-n, --no-mtab` | **accepted and ignored** (no mtab exists) — symmetric with the umount side (§2.6) |
| `--synth-sddl SDDL` | KACS synth-policy template SD (§8) |
| `-h, --help` / `-V, --version` | standard |

#### 2.5.1 Read-write and the auto-ro-fallback

Faithful to util-linux: when a read-write mount fails because the device is write-protected,
the tool retries read-only. Specifying `-w`/`--rw`/`-o rw` **forbids** this fallback (the
mount fails instead). `-r`/`-o ro` requests read-only directly.

#### 2.5.2 `--onlyonce`

Skips the mount if it is already present — but the check is **filesystem-driver-aware**, not
a naive source+target string match: it must account for the mount fs-root (so bind mounts
and btrfs subvolumes of the same device are distinguished) and must **not** block mounts the
driver legitimately permits more than once on the same point (e.g. multiple `tmpfs`). The
decision consults live mount state, not fstab.

### 2.6 `umount`

| Element | Spec |
|---------|------|
| `umount TARGET` / `umount SOURCE` | by mount point / by source (§2.6.2) |
| `-l, --lazy` | `MNT_DETACH` |
| `-f, --force` | `MNT_FORCE` |
| `-R, --recursive` | unmount each target and everything under it, **including over-mounted stacks** (util-linux ≥2.37); stops on first failure |
| `-A, --all-targets` | unmount **all** mountpoints of the given source in the current mount namespace (live mountinfo; the "unmount everywhere" complement to §2.6.2). **With `-R`:** compose the two — for *each* mountpoint of the source, recurse under it; stop on first failure (faithful to `umount(8)`). |
| `-d, --detach-loop` | free the backing loop device after unmount (§5.1) |
| `-r, --read-only` | on unmount failure, remount read-only (§6.7). **Mutually exclusive with `-R`** (util-linux enforces via `umount.c` `excl[]`/`err_exclusive_options`; combining → exit 1) |
| `-g, --graceful` | exit 0 if target is not mounted / absent (peios: unconditional, §1.4) |
| `-q, --quiet` | suppress "not mounted" messages |
| `-c, --no-canonicalize` | as mount; selects `UMOUNT_NOFOLLOW` (§2.6.1); applied unconditionally (§1.4) |
| `-v, --verbose` | as mount |
| `-N, --namespace NS` | as mount (§13) |
| `-n, --no-mtab` | accepted and ignored (no mtab) |
| `-i, --internal-only` | do not invoke a `umount.<type>` helper (accept/ignore; none ship) |
| `--fake` | dry run |
| `-h/--help`, `-V/--version` | standard |
| syscall | `umount2(2)` (§2.6.1) |

`umount -a`/`-O`/`-t` (fstab/batch filters) are **deferred** (§3). (`-A` is **kept** — it is
live-mountinfo, not fstab.)

#### 2.6.1 `umount2` flag mapping

`-l/--lazy`→`MNT_DETACH`; `-f/--force`→`MNT_FORCE`; non-canonicalized/untrusted target (`-c`
or symlink final component)→`UMOUNT_NOFOLLOW`. `MNT_EXPIRE` is **intentionally not
surfaced** — util-linux's CLI exposes no option for it either (internal libmount/autofs
mechanism).

#### 2.6.2 Ambiguous source

`umount SOURCE` matching multiple targets: refuse with an error naming the candidates unless
exactly one match exists. The user resolves the ambiguity with `-A` (unmount all) or by
naming a TARGET. A TARGET is always unambiguous.

---

## 3. Explicitly deferred / out of scope (DO NOT FLAG)

**fstab family (Ring 3):** `-a/--all`, `-F/--fork`, `-O/--test-opts`, `-T/--fstab`,
`--options-source[-force]`, `--options-mode`; one-arg fstab lookups; fstab-only `-o`
(`auto`/`noauto`, `user`/`users`/`nouser`/`owner`/`group`, `_netdev`, `nofail`); `-t` type
lists and `no<type>` negation **in the mounting path**; the `/proc/filesystems` probe
fallback; `umount -a`/`-O`/`-t`. (**`umount -A` is NOT here** — it is kept, §2.6. And the
no-operand **listing-filter** meaning of `-t` — including type-lists/`no<type>` — IS kept,
§11: filtering a materialized list is trivial and faithful, unlike batch *mounting*. `-O` is
NOT a listing filter — util-linux ignores it without `-a`, §11.)

**fstab comment fields (`x-*`/`X-*` persisted comments):** deferred with fstab. (Functional
`X-mount.*` options are classified in §6.9, not here.)

**mtab family:** `-n/--no-mtab` (accepted+ignored). Exit 16 never produced.

**Helpers & helper-only filesystems (§9 — mechanism kept, none shipped):** network fs
(`mount.nfs`/`mount.cifs`), FUSE, ntfs-3g; `-s/--sloppy`.

**Cut permanently (no peios analog):**
- SELinux `context=`/`fscontext=`/`defcontext=`/`rootcontext=` (KACS analog §8).
- `encryption=` (cryptoloop, removed from kernels).
- **`ID=` source tag** (udev hardware/WWN block-device ID). Depends on the udev hardware-ID
  symlink layer, is "not recommended for generic use" upstream, and has no concrete peios need.
  (The `-L`/`-U`/`LABEL=`/`UUID=`/`PARTLABEL=`/`PARTUUID=` source forms remain kept, §2.2.)
- **Idmapped mounts** (`--map-users`/`--map-groups`/`X-mount.idmap`/`MOUNT_ATTR_IDMAP`):
  idmap remaps POSIX uid/gid ranges to solve cross-userns sharing; peios solves that by
  adding a SID/ACE to the SD — a workaround for a limitation peios lacks.
- POSIX `X-mount.owner`/`group`/`mode`: peios ownership is SID/SD; the capability, if wanted,
  is a non-destructive superblock-scoped SD overlay (§8.6), not a chown.
- **Swap** (`-t swap`, swap devices): `swapon`/`swapoff`'s domain. `mount`/`umount` reject a
  swap target with a coherent error rather than probing/mounting it.

---

## 4. Argument-shape disambiguation

1. **No positionals, no verb** → list (§11).
2. **One positional, no `--source`/`--target`, no lone-target verb** → in util-linux this is
   *ambiguous* (device, target via fstab; or mountpoint, source+opts via fstab). peios has no
   fstab → **error (exit 1)** explaining a single operand cannot be resolved; supply both or
   `--source`/`--target`.
3. **Two positionals** → `SOURCE TARGET` (no fstab read).
4. `--source`/`--target` override; either may combine with one positional.
5. `-o remount` and a standalone `--make-*` take a lone **target**.
6. The **structural verbs** (bind / rbind / move / beneath / remount) are mutually exclusive
   with each other → exit 1.
7. **Propagation is NOT a mutually-exclusive verb.** `--make-*` / the `-o` propagation tokens
   may stand alone *or* accompany another verb or a new mount in the same command, applied as
   trailing `mount_setattr` propagation step(s) after the primary operation (§17) — faithful to
   util-linux (≥2.23), e.g. `mount --make-private --make-unbindable /dev/sda1 /foo` ≡ mount,
   then make-private, then make-unbindable. **Multiple** propagation changes in one command are
   likewise allowed (sequential `mount_setattr` calls, in order). Only the structural verbs in
   rule 6 conflict with each other.

`--target-prefix DIR` → `TGT := DIR + "/" + TGT` after the §4.1 decision, before the syscall.
`-m/--mkdir` creates the (prefixed) target if absent.

### 4.1 Canonicalization & path safety

Default: canonicalize source(path)/target (absolute, symlink-resolved); `-c` disables it (and
selects `UMOUNT_NOFOLLOW` for umount). The **mount side** mirrors umount's no-follow posture
(§17): use `move_mount`/`open_tree` resolve flags (no-follow on the final component for
untrusted/non-canonicalized targets) to harden against a TOCTOU symlink swap between
userspace resolution and the kernel syscall. Without mtab/fstab, canonicalization affects only
the path handed to the kernel and shown in output (weight grows with Ring 3); the kernel does
final resolution at syscall time.

---

## 5. Loop devices

Supported (pkm: `CONFIG_BLK_DEV_LOOP=y`, `/dev/loop-control`, `LOOP_*` incl. `LOOP_CONFIGURE`,
`lo_offset`/`lo_sizelimit`; no KACS gating beyond labelling).

- **Re-use first (corruption avoidance, util-linux ≥2.29):** before allocating, check whether
  the same backing file is already attached to a loop device with the **same offset and
  sizelimit**; if so, **reuse** that device rather than creating a new one (two loop devices
  over the same file region risks filesystem corruption).
- `-o loop` — otherwise auto-allocate a free device (`LOOP_CTL_GET_FREE`), attach
  (`LOOP_CONFIGURE` preferred; else `LOOP_SET_FD`+`LOOP_SET_STATUS64`), mount `/dev/loopN`.
- `loop=/dev/loopN` — use a specified device.
- `offset=N` / `sizelimit=N` — `lo_offset` / `lo_sizelimit`. **Imply loop**; an error with a
  block-device source (they are losetup options).
- **Implicit loop:** a regular-file source uses a loop device automatically **only when the
  type is unspecified OR the fs is recognised by libblkid** (matching util-linux — not "any
  regular file"). `X-mount.noloop` (§6.9) suppresses it.
- **Autoclear:** mount-created loops get `LO_FLAGS_AUTOCLEAR` so the kernel frees them on
  unmount (no leak).
- **Race:** on `EBUSY` from the `GET_FREE`→`CONFIGURE` window (device grabbed by a racing
  mount), retry with a fresh free device.
- On failure after attachment, detach (`LOOP_CLR_FD`) before returning. `encryption=` rejected.

`loop`/`offset`/`sizelimit` are category-(D); never forwarded to `fsconfig`.

### 5.1 `umount -d` vs autoclear

With `LO_FLAGS_AUTOCLEAR` set by default on mount-created loops, the kernel frees them
automatically on unmount, so `umount -d` is normally redundant. To avoid clearing a **recycled**
device (the number reused by another mount), `-d` is **best-effort and verified**: before
`LOOP_CLR_FD`, confirm the device still backs the source just unmounted; otherwise it is a
no-op. `-d` meaningfully force-clears only loops that lack autoclear.

**Lazy destruction (known limitation):** since Linux 3.7 `LOOP_CLR_FD` on a device that still
has open references only *marks* it for autoclear/lazy destruction — the `/dev/loopN` node may
linger briefly (visible to `losetup -l`, and a racing `LOOP_CTL_GET_FREE` may not yet see the
slot free; the §5 `EBUSY`-retry already absorbs this). The tool's responsibility ends at
`LOOP_CLR_FD` success; it does **not** block waiting for the node to fully disappear (no
`udevadm settle`-style wait). A caller that immediately reuses the freed slot may race — accepted.

---

## 6. The `-o` option table

### 6.1 Category A — per-mount attributes (`fsmount`/`mount_setattr` `MOUNT_ATTR_*`)

| Option | Attribute |
|--------|-----------|
| `ro` / `rw` | `MOUNT_ATTR_RDONLY` set/clear |
| `nosuid` / `suid` | `MOUNT_ATTR_NOSUID` |
| `nodev` / `dev` | `MOUNT_ATTR_NODEV` |
| `noexec` / `exec` | `MOUNT_ATTR_NOEXEC` |
| `noatime` / `atime` | `MOUNT_ATTR_NOATIME` / clear |
| `relatime` / `norelatime` | `MOUNT_ATTR_RELATIME` |
| `strictatime` / `nostrictatime` | `MOUNT_ATTR_STRICTATIME` |
| `nodiratime` / `diratime` | `MOUNT_ATTR_NODIRATIME` |
| `nosymfollow` | `MOUNT_ATTR_NOSYMFOLLOW` (no positive `symfollow` token — util-linux has none; the attr is cleared only via remount mask, §6.7) |

atime flags are constrained via the `MOUNT_ATTR__ATIME` mask, as in the kernel.

**Layer qualifiers:** `ro`/`rw` accept `=(recursive|vfs|fs)` as in current `mount(8)`:
`=recursive` applies recursively (`mount_setattr` + `AT_RECURSIVE`); `vfs`/`fs` select the
VFS-layer vs sb read-only scope; bare = vfs, non-recursive. Interaction with remount: §6.7.

### 6.2 Category B — superblock flags

`sync`/`async`→`SB_SYNCHRONOUS`; `dirsync`→`SB_DIRSYNC`; `lazytime`/`nolazytime`→`SB_LAZYTIME`;
`iversion`/`noiversion`→`SB_I_VERSION`; `silent`/`loud`→`SB_SILENT`.

> **Impl note (verify vs 7.0.9 headers):** exact new-API plumbing for Category B (fsconfig
> keys vs folded into fsmount) confirmed at implementation; behaviour normative, route not.
> `mand`/`nomand` removed from 7.0.9 — accept+ignore with a deprecation note.

### 6.3 Category C — fs-specific parameters (opaque passthrough)

Unrecognised tokens forwarded: `key=value`→`fsconfig(FSCONFIG_SET_STRING)`, bare
`key`→`fsconfig(FSCONFIG_SET_FLAG)`. No internal list; per-option `fsconfig` errors reported
(§10). **No option-string length limit on the `fsconfig` path** (the classic `mount(2)`
PAGE_SIZE data ceiling applies only if the §1.3 classic fallback is ever activated).

### 6.4 Category D — userspace-only

`loop`/`offset=`/`sizelimit=` (§5); meta-verbs `remount`/`bind`/`rbind`/`move`; the
**propagation `-o` tokens** `shared`/`slave`/`private`/`unbindable` and their recursive
spellings `rshared`/`rslave`/`rprivate`/`runbindable` (util-linux accepts these as `-o`
options, incl. on the command line — the `r`-prefix is the recursive form, distinct from the
hyphenated `--make-r*` flags) → route to the propagation operation (§2.1: `mount_setattr`
`propagation` field; `r`-prefix adds `AT_RECURSIVE`); `defaults` (§6.6). Never forwarded to
`fsconfig`.

### 6.5 Category E — KACS mount policy

`policy=<kind>` (§8); template via `--synth-sddl`. Never forwarded.

### 6.6 `defaults`

Expands to **`rw,suid,dev,exec,async`** (util-linux's `rw,suid,dev,exec,auto,nouser,async`
minus the fstab-only `auto`/`nouser`; **no `relatime`** — it is not part of `defaults`,
despite being the kernel runtime default). Later tokens override earlier.

### 6.7 Remount (per-layer; no flag-preservation needed on the new-API path)

**Key correctness point — and a refinement of the "faithful to util-linux" principle:** the
classic `mount(2)` `MS_REMOUNT` path **resets** unspecified VFS flags, which is *why*
util-linux reads `/proc/self/mountinfo` and re-supplies the unchanged ones. **The new-API
path has no such reset**, so that preserve-dance is *unnecessary* for us — replicating it
would be porting a workaround for an old-API limitation we do not have (§1.5 generalized:
do not mirror util-linux behaviour that only exists to compensate for `mount(2)`'s
shortcomings). Remount is therefore **per-layer**, each layer touching only what was named:

- **Per-mount (VFS) attrs** (`ro`/`rw` bare or `=vfs`, `nosuid`/`nodev`/`noexec`/atime
  family/`nosymfollow`) → `mount_setattr` with the `attr_set`/`attr_clr` **mask**: only the
  named bits change; every other attr (and every submount, under `AT_RECURSIVE`) is left
  untouched by the kernel. No read-and-preserve required. atime is set via the
  `MOUNT_ATTR__ATIME` mask.
- **Superblock flags / fs params** (Category B/C) **and `ro=fs`/`rw=fs`** → `fspick` +
  `fsconfig` (`FSCONFIG_CMD_RECONFIGURE`): starts from the live superblock state and applies
  only the deltas you set — it does **not** reset to mount-time defaults. **The `=fs`
  read-only qualifier targets the *superblock* (`SB_RDONLY`), which per-mount `mount_setattr`
  cannot set — so `ro=fs`/`rw=fs` on remount route here, not through `mount_setattr`.** (Bare
  `ro`/`rw` and `=vfs`/`=recursive` are per-mount and route above.) Note: `FSCONFIG_CMD_RECONFIGURE`'s
  atomicity *across multiple set options* is filesystem-driver-dependent (not a kernel
  guarantee like `mount_setattr`'s); on failure report the per-option `fsconfig` key+message
  (§10) and do not assume partial-option non-application.
- **Bind mounts remount per-mount-only:** a bind shares the source's superblock, so a remount
  of a bind MUST go through `mount_setattr` (per-mount) and MUST NOT touch the sb via
  `fsconfig` — otherwise it would alter the shared superblock for every mount of that fs.
  Consequently we treat **any superblock-level option on a *bind* remount as a usage error
  (exit 1)** — this covers `ro=fs`/`rw=fs` *and* every Category-B flag (`sync`/`dirsync`/
  `lazytime`/`iversion`/…) *and* Category-C fs-specific params, since all of them would require
  reconfiguring the shared sb. This is a deliberate **userspace policy guard**, not a kernel
  constraint: the kernel would *not* refuse it (`fspick` on the bind's target picks up the shared
  underlying sb and would happily reconfigure it for every mount of that fs) — and util-linux
  itself does *not* guard this — so the tool catches it to avoid the silent shared-sb mutation
  (§1.5). Only per-mount VFS attrs (bare/`=vfs`/`=recursive` `ro`/`nosuid`/…) are valid on a
  bind remount.
- **`=recursive` on remount is supported** (`mount_setattr` + `AT_RECURSIVE`): the mask model
  applies the named attr to the whole subtree, each submount keeping its other attrs. (The
  old worry that mountinfo shows only the top mount's flags is moot — the kernel masks
  per-submount, so nothing needs preserving.) **All-or-nothing:** the kernel validates the
  entire subtree under the namespace lock before committing (`do_mount_setattr`'s
  `mount_setattr_prepare`→`mount_setattr_commit`: prepare validates the whole tree and writes
  nothing, commit cannot fail), so a recursive `mount_setattr` that fails on any submount (e.g.
  a locked attr → `EPERM`/`EBUSY`) applies to **none** of it. On failure, surface exit 32 with
  the offending mount identified where the kernel reports it; the tool guarantees no partial
  application (§1.5). *(This guarantee is verified against kernel source — it is NOT stated in
  the `mount_setattr(2)` man page — so confirm with a provium test at implementation.)*
- **Classic-fallback only:** the read-mountinfo-and-re-supply-unspecified-flags behaviour is
  retained *solely* for the §1.3 classic `mount(2)` fallback path, where the reset does occur.

### 6.8 Attr flags on a bind (`-o bind,ro`)

`open_tree(OPEN_TREE_CLONE)` → apply requested `MOUNT_ATTR_*` via `mount_setattr` on the clone
→ `move_mount` (the new-API equivalent of util-linux's bind-then-remount, ≥2.27).
`=recursive` applies with `AT_RECURSIVE`.

### 6.9 Category F — functional `X-mount.*`

| Option | Disposition | Behaviour |
|--------|-------------|-----------|
| `X-mount.mkdir[=mode]` | **K** | create target dir (alias of `-m`) |
| `X-mount.subdir=DIR` | **K (new-instance mounts only)** | attach subdirectory `DIR` of a freshly-mounted filesystem at the target — `open_tree(DIR, OPEN_TREE_CLONE)` → `move_mount` (clean on 7.0.9's FD API). **Effective only when a new filesystem instance is attached; silently ignored for bind/rbind/move/remount/propagation** — faithful to upstream (man page + libmount `is_subdir_required()` flag check). For binds, put the subpath in the SOURCE instead. (Surface the ignore under `-v`.) Upstream's mtab-update wart is N/A here — no mtab. |
| `X-mount.noloop` | **K** | suppress implicit loop (§5) |
| `X-mount.auto-fstypes=LIST` | **K** | constrain `-t auto` probing (§7) |
| `X-mount.nocanonicalize[=type]` | **K** | `-o` form of `-c`; the optional `=type` is **`source`** or **`target`** (per `mount(8)`), selectively disabling canonicalization for just that path; omitting `=type` disables it for both. (Not `all`/`mtab`/`fstab` — those were wrong.) |
| `X-mount.idmap=…` | **D (cut)** | idmap — §3 |
| `X-mount.owner`/`group`/`mode` | **D (cut)** | POSIX chown — §3; future SD-overlay §8.6 |

**`open_tree` composition:** `subdir` and `rbind` are **mutually-exclusive contexts** —
`subdir` applies only to a *new-instance* mount, while `rbind` is a *bind* operation (where
`subdir` is ignored, above) — so there is no ambiguous "recursive bind of a subdir" to
adjudicate. A bind invocation composes CLONE (§6.8) + optional AT_RECURSIVE (§2.1 rbind) +
optional `mount_setattr` attrs; ordering is clone → `mount_setattr` → `move_mount`. A
new-instance `subdir` mount composes `open_tree(DIR, CLONE)` → optional `mount_setattr`
attrs → `move_mount`.

---

## 7. Source resolution & type probing (libblkid)

- **libblkid** (util-linux source, LGPL-2.1+), vendored standalone, dynamically linked; MIT
  consumer unaffected; soname dep auto-derived.
- `-U`/`-L`/`UUID=`/`LABEL=`/`PARTUUID=`/`PARTLABEL=` → blkid lookup → device; no match → exit
  32.
- `-t auto`/omitted → `blkid_do_safe_probe`-style **safe** probe (refuse on ambiguous/multiple
  signatures → exit 32). `X-mount.auto-fstypes` constrains candidates. Endianness/signature
  handling is internal to libblkid.
- Pseudo sources skip probing; `-t` required (cannot probe `none`).

---

## 8. KACS mount policy (Ring 2)

### 8.1 CLI

```
mount -o policy=deny-missing      SRC TGT
mount -o policy=synth-ephemeral   SRC TGT
mount -o policy=synth-persist     SRC TGT
mount -o policy=synth-persist --synth-sddl 'O:..G:..D:(..)' SRC TGT
```

`deny-missing`/`synth-ephemeral`/`synth-persist` → `KACS_MOUNT_POLICY_DENY_MISSING`/
`…SYNTHESIZE_EPHEMERAL`/`…SYNTHESIZE_PERSISTENT`. `unmanaged` is not user-settable.

### 8.2 Validity

- `policy=` valid only on a **new mount** of a real (FACS-capable) fs; usage error (exit 1)
  with bind/rbind/move/beneath/remount/propagation/list.
- `--synth-sddl` valid only with `synth-*`; else exit 1.
- Mount policy is **superblock-scoped**, not per-mountpoint (documented in `--help`). A block
  device has one shared superblock (kernel refuses a second writable instance), so all mounts
  of that data share one policy.

### 8.3 Atomic set-before-attach flow (no rollback)

```
fd = fsopen(type); fsconfig(fd, …); mnt = fsmount(fd, attrs)   // DETACHED
kacs_set_mount_policy(mnt, {policy, template_sd?})
   ├─ success → move_mount(mnt → target)
   └─ failure → close(mnt); DO NOT move_mount                  // sb torn down
```

Never visible with an unintended policy; no rollback. `kacs_set_mount_policy` resolves the sb
via `fget_raw` (accepts the O_PATH-style `fsmount` fd) — **verify with a provium test**; if
not, fall back to mount→set→`umount`-on-failure. No `policy=` → kernel default (fail-safe
`DENY_MISSING` for storage fs).

### 8.4 Client-side pre-validation

Reject `--synth-sddl` with non-synth kinds; parse via `security::sddl`, require an **owner**;
reject SD > `PKM_KACS_MAX_SD_BYTES`. No `SeTcbPrivilege` precheck (set-before-attach makes
`EPERM` clean).

### 8.5 Error translation

`EOPNOTSUPP`→"filesystem type '<T>' does not support mount policies" (UNMANAGED; don't
reimplement the magic table); `EPERM`→"requires SeTcbPrivilege"; `EINVAL`→from the validated
cause.

### 8.6 Future: SD-overlay (not v1)

The capability behind POSIX `X-mount.owner/group/mode` (§3), if wanted, is a non-destructive
superblock-scoped SD overlay carried by the mount policy — SID-native, not a chown.

---

## 9. Helper mechanism (kept; zero helpers shipped)

For a type `T` not handled internally, look for `mount.<T>`/`umount.<T>` in the helper path;
if found and `-i` unset, `exec` it (`mount.<T> SRC TGT [-fnv] [-o OPTS]`). v1 ships none, so
network/FUSE types fail with "no helper for type <T>" (exit 1) — cleanly deferred. `-i` forces
internal handling; `-s/--sloppy` not implemented (§3).

---

## 10. Error model & exit codes

Follows util-linux `mount(8)`; deferred-feature code 16 (mtab) never produced. Code **64**
("some succeeded, some failed") arises only from **additive accumulation across multiple
target arguments** — the `-a` path (deferred, §3) and the outer `umount SRC1 SRC2 …` /
`umount -A SRC1 SRC2 …` loop, which does `rc += …` per arg (faithful to util-linux). A
*single* `-A`/`-R`/`-A -R` invocation uses a **stop-on-first-failure** contract → exit 32 on
the first failure (never an aggregate); 64 only appears when several source arguments are
given and at least one succeeds while another fails.

| Code | Meaning | peios mapping |
|------|---------|---------------|
| 0 | success | — |
| 1 | incorrect invocation **or permissions** | arg/usage errors, mutually-exclusive verbs, invalid `policy=`/`--synth-sddl`, authz denials (`EPERM`/`EACCES`), single-positional-without-fstab, "no helper for type", embedded-NUL path |
| 2 | system error | `ENOMEM`, cannot fork, no free loop device |
| 4 | internal bug | invariant failures |
| 8 | user interrupt | SIGINT (with signal-safe cleanup, §17) |
| 32 | mount failure | `fsopen`/`fsconfig`/`fsmount`/`move_mount`/`open_tree`/`fspick`/`mount(2)` failure; libblkid not-found/undetermined; `--beneath`/move/`--exclusive` kernel refusals; recursive-remount partial-failure (all-or-nothing, §6.7) |
| 126 | helper exec failure | a found `mount.<type>` helper that fails to `execl` (util-linux ≥2.41) — moot until a helper ships (§9), enumerated for symmetry with `umount` |

`umount` exit codes: 0; 1 usage/permission; 2 system error; 32 unmount failed; **126** if an
external `umount.<type>` helper fails to execute (kept-mechanism path, §9 — moot until a
helper ships). `-g/--graceful` forces 0 when the target is absent/not-mounted.

Message style: faithful util-linux phrasing; new-API per-option failures use the `fsconfig`
key+message; KACS uses §8.5. The internal structured error enum maps onto these codes.

---

## 11. List mode output

- `mount` (no args): one line per mount, `SOURCE on TARGET type FSTYPE (OPTIONS)`.
- **Listing filter:** `mount -t TYPE` with **no operands** filters the displayed list by fstype
  rather than mounting — faithful to `mount(8)` (`print_all` uses `mnt_match_fstype`). Type-lists
  and `no<type>` negation (`-t ext4,xfs`, `-t nosysfs`) are honoured here (trivial on a
  materialized list — distinct from, and not blocked by, the mounting-path type-list deferral in
  §3). **`-O` does NOT filter the listing** — util-linux ignores `-O` in list mode ("-O is useless
  without `-a`"), so `mount -O …` with no operands lists everything; do not implement an `-O`
  list filter. `-t` given *with* operands is the mount type-specifier.
- Source: `/proc/self/mountinfo` (or `statmount(2)` where available). Reading is **not atomic**
  — tolerate torn snapshots (concurrent mount/umount can drop/duplicate lines or show a child
  before its parent); re-read on detected inconsistency, as util-linux does.
- The `(OPTIONS)` field is the **positional VFS→FS→user option merge** (per-mount/vfs options,
  then superblock/fs options, then user options) with **ro/rw coalescing** — **not** sorted or
  canonically reordered. (The example `rw,relatime,seclabel` matches because mountinfo already
  emits that order.)
- **Field decoding:** apply mountinfo **unmangle** (octal-escape decode, `\040`→space, etc.) to
  paths/options before merge; replace control characters in the **mountpoint** with `?`
  (matching `mount(8)`; source/fstype/options are printed raw, as util-linux does). The
  **source** is shown as the kernel recorded it in mountinfo (already a device path for block
  filesystems — not a `LABEL=`/`UUID=` tag); `mnt_pretty_path`-style resolution runs full
  `realpath()` on the source (resolving *any* symlink, not just `/dev/disk/by-*`), and also maps
  `/dev/loopN`→its backing file and `/dev/dm-N`→`/dev/mapper/<name>`. There is no tag→device
  evaluation at list time (that happened at mount time).
- `-l/--show-labels` appends ` [LABEL]` (bracketed, space-separated, after `(OPTIONS)`); no
  label → no suffix (not `[]`).
- **KACS-filtered view:** pkm gates `/proc/<pid>/mounts*` (`proc_namespace-mount-metadata-gate
  .patch`); the listing shows only mounts the caller's token may observe — correct, honest
  behaviour, no error on a partial/empty view (§1.5). **Information-leak rule:** when a row's
  `parent ID` refers to a mount that is filtered out, do **not** reconstruct a path/tree that
  reveals the hidden parent's existence or path — render the row root-relative or omit it.
- `mount --tree`/`--list-fulltree` do not exist (findmnt features); correctly absent.

---

## 12. `--fake` and `--verbose`

- `-f/--fake` (mount) / `--fake` (umount): full parsing, canonicalization, libblkid resolution,
  loop *planning* (no attachment), option partitioning; **skip** the mount syscalls and
  `kacs_set_mount_policy`. Report under `-v`.
- `-v/--verbose`: narrate resolved values + each syscall to stderr. `-vv` ≈ `-v`.
- **fs_context kernel message log:** drain and print the new-API `fs_context` log. Under `-v`
  this includes the success-path detail; **on an `fsconfig`/`fsmount`/`fsopen` *failure* the
  log is drained and reported regardless of `-v`**, because it is often the only explanation
  for *why* the mount failed — gating that behind `-v` would hide the error reason. **Buffer
  policy:** reading the log returns `EMSGSIZE` *and consumes the message* if the read buffer is
  too small, permanently losing it — so the drain MUST use an adequately large / dynamically
  resized read so no message is lost to `EMSGSIZE`.

---

## 13. Namespaces (`-N`)

`-N NS` accepts a **PID**, an **ns file path** (`/proc/<pid>/ns/mnt`), or a **named ns**.
**Ordering:** source/UUID resolution and canonicalization happen in the **origin** (caller's)
namespace; `setns()` into the target mount ns happens **before** the mount syscall — so
libblkid (§7) probes the origin's `/dev` and the mount lands in the target ns. pkm's
mount-namespace **authz** is not yet fully implemented (falls back to the privilege→capability
map), so cross-ns behaviour may be clumsy — substrate immaturity surfaced honestly (§1.5).

---

## 14. Environment

The tool **does not link libmount**, so `LIBMOUNT_*` variables (incl. `LIBMOUNT_FORCE_MOUNT2`,
`LIBMOUNT_FSTAB`, `LIBMOUNT_DEBUG`) have no effect (documented, not silently ignored). No
`--force-mount2` override in v1 (§1.3 per-fs fallback suffices). libblkid's `LIBBLKID_DEBUG`
passes through to the linked library. `LOOPDEV_DEBUG` not honoured.

---

## 15. Audit instructions

Confirm each item is **(K)** kept faithfully or **(D)** in §3. Flag **GAP** (neither) and
**DIVERGENCE** (kept but wrong). §3 items are intentional — not gaps.

1. Every `mount`/`umount` command-line flag: K or D?
2. Every `-o` filesystem-independent option: §6 (K) or §3 (D)?
3. Argument-shape disambiguation (§4): faithful for kept shapes?
4. Source forms & safe-probe (§2.2/§7)?
5. Loop (§5): re-use, implicit-loop trigger, autoclear, `-d` reconciliation, EBUSY-retry,
   detach-on-failure, offset/sizelimit-imply-loop?
6. Verbs (§2.1): bind/rbind/move/beneath/remount/propagation/new — mechanism + faithful CLI;
   remount per-layer + bind-per-mount-only (§6.7); bind attrs (§6.8); `open_tree` composition
   (§6.9); `--beneath`/move/`--exclusive` constraints + propagation defaults?
7. Exit codes (§10), incl. umount 126; deferred codes never produced?
8. List output (§11): positional merge (not sorted), unmangle/control-char/source-tag,
   torn-read, parent-filter leak, `[LABEL]`?
9. `umount` (§2.6): full flag set incl. `-A`/`-R` over-mount; `umount2` flags; ambiguous source?
10. `X-mount.*` (§6.9): functional ones classified; combination rules?
11. peios specifics (§8 policy, §13 ns, §11 filtering, §3 cuts, §1.4/§1.8): additive,
    non-breaking, cuts justified?
12. Robustness (§17): signal-safe cleanup, TOCTOU/no-follow, byte handling?
13. Divergences justified? Each traces to fstab/mtab absence, helper deferral, the new-API
    mechanism, the KACS/SID model, or a documented §1.4 privilege divergence — never an
    unexplained omission.

Output: rows of (item, K | D | GAP | DIVERGENCE, note with citation). GAPs/DIVERGENCEs first.

---

## 16. Library architecture

The mount/umount logic is a **clean library crate** (peios-native reimplementation of
libmount's role, minus fstab/mtab); the applets are **thin CLI wrappers**. Cheap insurance:
a single testable core now, and a deferred choice later — when an external consumer of
`libmount.so.1` appears (findmnt, udisks2, some systemd paths; less universal than libblkid
and fstab/mtab-centric, which is why we reimplement rather than vendor), expose the core via a
**peios-native `extern "C"` shared library** (libpeios-style, serving libmount's role, not a
byte-compatible clone) in preference to vendoring real libmount. Only the library-crate
structure is committed in v1.

---

## 17. Robustness & safety

- **Signal-safe cleanup of multi-step flows.** The new-API flows are multi-syscall
  (`fsopen→fsconfig→fsmount→move_mount`; loop `GET_FREE→CONFIGURE→mount`; bind
  `clone→mount_setattr→move_mount`). On SIGINT (exit 8) **before the mount is attached**
  (`move_mount` not yet done), perform best-effort cleanup: `close()` any detached `fsmount`
  fd (the kernel tears down the unattached superblock), and `LOOP_CLR_FD` any loop device
  attached but not yet handed to a successful mount — so an interrupt leaks neither an
  invisible superblock nor a loop device. (Once `move_mount` has attached the mount, an
  interrupt leaves it in place, same as any completed mount — there is nothing to unwind.)
- **TOCTOU / no-follow.** Use resolve/no-follow controls on the final path component for
  untrusted or non-canonicalized targets (mount side), symmetric to `UMOUNT_NOFOLLOW` (§2.6.1),
  to resist a symlink swapped in between userspace resolution and the kernel syscall.
- **Byte handling.** Per §1.8: opaque bytes end-to-end; reject embedded NUL.
- **Option-string length.** No limit on the `fsconfig` path (§6.3).
- **Combined verb + propagation partial failure.** `mount <verb> … --make-private` is *two*
  steps (the verb's `move_mount`, then a `mount_setattr` propagation call, §2.1) and is not
  atomic across the pair. If the verb succeeds but the `mount_setattr` propagation step then
  fails, follow util-linux: **leave the mount in place and report the propagation failure
  separately** (non-zero exit), rather than unwinding the successful mount — the mount itself is
  valid; only the propagation attribute didn't apply. (Distinct from the pre-attach SIGINT case
  above, where the mount was never published.)
