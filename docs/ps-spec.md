# peiosutils `ps` — Implementation Specification

Status: **DRAFT v1.3 — design only, three audit rounds folded in (round 3 caught the codebase-reuse + interaction gaps the prior axes missed); not yet implemented.** Captured from an interactive design session (decisions tagged `D1`–`D21` + principles `P1`/`P2`). Revision history in §0.

This spec defines the v1 behaviour of the `ps` applet in peiosutils (a uutils/coreutils-derived multicall binary).

The design intent is **faithful reproduction of procps-ng `ps(1)` behaviour**, restricted to the surface defined here, and adapted to the peios kernel (pkm, Linux 7.0.x base) and its security model — KACS, Security Descriptors, the process **token** (user SID + integrity level), the **PSB** (Process Security Block: PIP trust label + mitigations), and impersonation. Every point where peios **diverges** from procps is called out explicitly as a *divergence* with rationale; every divergence is gated by compat mode (§12). Anything in procps `ps` not listed as deferred (§15) and not specified as kept here is a spec gap to be flagged. Audit instructions are in §16.

> **`ps` is built substrate-first.** Its peios security surface — and the default view, since `aag` (§4.1) needs `/pip` — depends on the §13 kernel/libpeios substrate, which does **not** yet exist. The OS author delivers the full §13 surface **before** ps implementation begins (the classic Linux columns could be built standalone, but the intended order is substrate-first so the default view works out of the box). At runtime, per-process access-gated fields still honest-degrade per P2 (§11).

---

## 0. Revision history

**v1.3 — round-3 audit (implementability/interactions + cold-generalist codebase-reuse); caught the real gaps the prior axes shared a blind spot on:**
- **§1.3 / §5.2 / §13.2:** corrected the `sid_render` over-claim — it is **net-new design, not libpeios reuse**. The only existing renderer is the applet-private `token/sid_render.rs` (NT-SID table only; no `*` sigil, no `/run/resolve-sid.sock`, no integrity/PIP tables). The libpeios work-list now scopes the sigil scheme, the socket protocol, and the integrity+PIP label tables as unbuilt, with `token/sid_render.rs` as the promote-and-extend starting point.
- **§1.5 / §9 / §10:** corrected the lsblk-engine over-claim — the tree walk/glyphs, width/padding, and `cmp_by_col` transfer (after a trait extraction to uucore); `render_json`/`render_pairs` are **re-implemented** (lsblk's are `blockdevices`-hardwired), and the `aag` flag column + thread/forest dual-nesting are net-new.
- **§3 / §3.1 (decision):** a lone filter (no selector) filters the **default own+tty baseline** — the default selector always applies when no selector is given; **no implicit `-e`** (on peios most processes carry some PIP trust, so baseline∩filter is useful; `-e` is explicit for system-wide).
- **§6.5 / §11 (decision):** **partial `aag`** — render the flags that could be computed, then append a trailing `?` (partial enumeration). Fully-unreadable = `?`; fully-read baseline = blank. Never silently drop a flag.
- **§10 (decision):** full JSON schema pinned (root `{"processes":[…]}`, lowercase-keyword fields, JSON numbers for numeric columns, `null` for unreadable, threads under `"threads"`, forest children under `"children"`, empty `aag` = `""`).
- **Intro / §13:** build order is **substrate-first** — the OS author delivers the full §13 surface before ps implementation; the buildable-now/gated split is superseded.
- **New §4.3 + scattered:** interaction/precedence rules pinned procps-faithful (`-o` replaces / `-O` appends; multiple presets last-wins; `-o` unknown = error; default sort = PID order; `-N` negates the selector union before filters; classic columns degrade as procps; `--impersonating` under `H` filters at the real-process level; the BSD pre-tokenizer emits a normalized UNIX argv into clap).

**v1.2 — confirming audit round (substrate re-verify + consistency/faithfulness re-verify); clean on both axes. Residual polish only:**
- §12: the disabled-filters list made exhaustive — *all* peios security filters (`--integrity`/`--pip`/`--pip-type`/`--pip-trust`/`--mitigation`/`--impersonating`) are disabled under compat (was a partial list that could read as letting `--impersonating` survive).
- §13.1: noted the integrity file synthesizes `S-1-16-<rid>` from the token's `integrity_rid` (the token stores a RID, not a pre-formed SID).
- §6.6: clarified `/sids` is the selector-backing file, not a §6 column.
- §0 wording tidy. Substrate confirming round verified all five v1.1 fixes correct against `pkm/uapi`, `pkm/kacs`, `kacs-core/src/pip.rs`, and the learn catalog; faithfulness round verified the procps fixes live against procps-ng 4.0.4.

**v1.1 — three-axis adversarial audit (substrate-realism / procps-faithfulness / internal-consistency); all findings folded in. Every numeric/factual substrate claim was confirmed accurate — the fixes are framing, consistency, and faithfulness.**
- **Appendix D / §6.3 (BLOCKER):** PIP catalogue re-framed — the trust tiers bind to **type 512 (Protected)**, and `Isolated` is `S-1-19-1024-8192` (reserved). The numeric values were already correct; the "two independent axes" framing was not (a renderer would have invented a non-existent `Isolated-1024`).
- **§13.1 / §13.3 (BLOCKER):** the PSB read-path gap applies to **`pip` *and* `mitigations`** (both are PSB fields; the only PSB syscall is `set_psb`=1005, set-only). The token read-path supplies only `sid`+`integrity` (token fields); `pip`/`mitigations` need a new cross-process PSB getter. The cited learn doc describes a *self-only, token-based, `QUERY_INFORMATION`-gated* getter, not our cross-process procfs file — corrected.
- **§5.1 / §13.1 / §3.2:** defined **True SID** (= the process's primary-token user SID); `/proc/<pid>/sid` = True SID (v1.0 mislabeled it "effective"), `task/<tid>/sid` = per-thread effective.
- **§6.5 / §8:** in thread mode only `i`/`I`/`S` (integrity/confinement, token-borne) vary per-thread; `t`/`T` (PIP) stay process-level (PIP is impersonation-invisible — confirmed by audit).
- **§6.5 / Appendix B:** dropped the false "upper = elevated" case invariant (`t` = protected is lowercase yet elevated). Each flag is documented explicitly instead. `aag` baseline stays **blank** — a deliberate exception to §11's `-`, for at-a-glance scannability; JSON-empty `aag` = `""`.
- **§3.2 / §8 / §15:** `--integrity` and `--impersonating` are both **v1 filters**; thread-mode display is *complementary* to the `--impersonating` filter, not a replacement. `--impersonating` value grammar specified.
- **§12:** `--json`/`-P` are peios-wide additive output and are **not** reverted by compat; `--sid` under compat = session selector.
- **§7 / §9 / §3:** `-F` column order corrected (`SZ RSS PSR` insert after `C`); sort = `--sort`/`k` (`-x` kept as a *flagged* peios/lsblk-consistent alias); `-a` = has-tty **and** not-session-leader; the `-g`/`-G` collapse is now a flagged divergence (procps `-g` = session/effective group, `-G` = real group).
- Minors: `-ww` (unlimited width); `Untrusted(0)` integrity level added; the `--sid`-anywhere vs `-o sid`-user scope footgun documented; §1.2 scoped to *output*; the default selector's internal AND noted.

**v1.0** — initial design from the interactive session (D1–D21).

---

## 1. Design principles

1. **Faithful where it can be, peios-native where it matters.** The classic ps surface (selection, classic columns, presets, threads, forest, sort) reproduces procps. The identity/security surface is re-expressed in peios terms (SIDs, integrity, PIP, mitigations) rather than POSIX uid/gid/mode.
2. **No uid leakage outside compat (D15).** A projected uid is a Linux-compatibility shim. It never appears in default or preset **output**; the only native way to *display* it is an explicit `-o uid`. (A uid may still be given as *input* to `-u`/`-U` — that's a selection argument, not a leak.) Under `PEIOS_COMPAT_MODE=LINUX` (§12) identity reverts to Linux shapes.
3. **One canonical SID renderer (D6).** All SID→display rendering goes through a single `sid_render` (§5.2), so `ls`/`lsblk`/`ps` render identities identically and authd lights them all up at once. This is **to-be-built** — promoted to libpeios from the applet-private `token/sid_render.rs` and extended (sigil, resolver socket, integrity/PIP tables) — not existing reuse.
4. **Honest per-field degrade (P2, §11).** `ps` never aborts a listing because one field of one process was unreadable. A process always appears (PID is from `getdents`, not gated); each unreadable field shows `?`.
5. **Build on the lsblk output engine.** The table/tree renderer, column width, tree glyphs, and `cmp_by_col` are extracted from `lsblk`'s `output.rs` into uucore via a `Row`/`Column` trait and shared (D18–D21). The **JSON/pairs renderers are re-implemented** (lsblk's are hardwired to its block-device shape — a template, not a drop-in); the `aag` flag column and the thread/forest dual-nesting are net-new.

---

## 2. CLI syntax model (D1)

`ps` accepts **three option styles**, scoped as **"B+"**:

- **UNIX** (dash-prefixed, clustered): `ps -ef`, `ps -el`, `ps -o pid,user`.
- **GNU long**: `ps --sort=-pcpu`, `ps --forest`.
- **BSD** (no dash) — **curated subset only**, not full dash-sensitive BSD. A pre-tokenizer in front of clap recognizes the common BSD forms `aux`, `ax`, `au`, `u`, `ux`, `r`, `x`, and the arg-taking selectors `p <pid>`, `t <tty>`, `U <user>`.

**Divergence:** full faithful dash-sensitive BSD (the procps "personality" parser, ~40 letters each with two meanings) is **deferred** (§15). The pre-tokenizer maps the curated BSD forms to their UNIX-equivalent behaviour. Unrecognized barewords are resolved per §2.1.

### 2.1 Bareword resolution order
A non-option operand resolves in this fixed order: **real flag → numeric (BSD pid) → known BSD cluster → registry user-macro (§14, D16) → error.** Built-in syntax always wins; a macro can never shadow a real flag or `aux`.

The pre-tokenizer runs on the leading bareword(s) and **rewrites recognized BSD forms into a normalized UNIX argv** that clap then parses — e.g. `ps aux -o pid` → the `-e` selection + the `u` format + `-o pid`; `ps U root` → `ps -U root`; `ps p 123 -f` → `ps -p 123 -f`. Options after the BSD cluster are parsed normally by clap.

---

## 3. Process selection

Selection has **two tiers** (D12), combined as `result = union(selectors) ∩ AND(filters)`. The **default selector** (§3.1) is what `union(selectors)` resolves to when *no* selector is given — so a filter-only invocation (`ps --pip`) narrows the **default own+tty baseline**, *not* all processes (there is **no** implicit `-e`; use `-e` explicitly for system-wide). On peios most processes carry some PIP trust, so a baseline∩filter result is useful rather than empty.

### 3.1 Selectors (union — procps-faithful)
A process is selected if it matches **any** selector (additive/OR, exactly like procps). Default (no selector): processes with the **same security SID as the caller** (D2 — "mine" = same SID, read cheaply from `/proc/<pid>/sid`) **and** on the caller's controlling terminal. (The default is itself a conjunction — same-SID *and* same-tty — a procps-faithful exception to the otherwise-union selector tier.)

| Selector | Selects |
|---|---|
| `-e` / `-A` | all processes |
| `-a` | all with a controlling terminal **and** not a session leader (both constraints, procps-exact) |
| `-d` | all except session leaders |
| `-p` / `p` / `--pid` | by PID list |
| `--ppid` | by parent PID |
| `-C` | by command name |
| `-t` / `t` / `--tty` | by controlling terminal |
| `-s` / `--sesid` | by **session** id (D7 — see §5.1) |
| `-u` / `-U` / `--user` / `--User` | by user (see §3.3) |
| `-g` / `-G` | by group (see §3.3) |
| `-N` / `--deselect` | negate the selection |

### 3.2 Filters (AND — narrowing; peios extension)
Filters narrow the selected set; **all** must hold (intersection). Within one filter, comma = OR (any-of); repeated/separate filter flags = AND. *"Commas widen, repeats narrow."*

| Filter | Keeps processes where |
|---|---|
| `--sid <SID>` | the process **carries** this SID anywhere in its security context (any class — user/group/integrity/PIP/confinement/capability), via `/proc/<pid>/sids` |
| `--integrity <LEVEL>` | integrity level matches |
| `--pip [<label>]` | bare = any PIP protection (`type≠None`); value = exact label |
| `--pip-type <type>` / `--pip-trust <tier>` | per-axis PIP match (two-axis; subsumes globs) |
| `--mitigation <code>` | has this mitigation (repeat = AND; `--mitigation WXP --mitigation TLP` = has both) |
| `--impersonating [<sid>]` | **opt-in security audit (v1 filter)** — keeps processes with a thread whose effective SID ≠ the **True SID** (§5.1). Bare = any impersonation; a value/comma-list matches a thread acting as any of those SIDs; repeated narrows (per the rule above). Walks `task/<tid>/sid` (heavy, only when requested; never hot-path). Thread mode (§8) shows *which* thread |

**Divergence:** procps unions *everything*. peios unions selectors but ANDs the (new) filters. The classic selectors keep procps union semantics, so existing ps idioms are unchanged. Compat (§12) forces flat-union everywhere.

### 3.3 Identity selectors (D14)
- `-u` / `-U` are **aliases** (match the user SID). On peios there is no euid/ruid split — the POSIX real/effective distinction is a per-*thread* impersonation concept (§8), not a per-process pair, so the two collapse. They accept a **username, uid, or SID**, resolved to a SID for matching (name needs authd; uid and SID work today).
- `-g` / `-G` are aliases (match group SIDs); accept **group name, gid, or SID**. **Divergence:** procps `-g` = session *or* effective group and `-G` = real group; peios collapses them (no real/effective group split, same reasoning as `-u`/`-U`) and drops procps's `-g`=session-group wrinkle (use `-s`/`--sesid` for session). Compat keeps both matching group identity.
- The freed uppercase case is **not** repurposed for broad-class matching (the "identity-esque vs group-esque" bucketing is ill-defined for confinement SIDs); broad matching is `--sid`-anywhere instead.

---

## 4. Columns

### 4.1 Default column set (D5, D13)
```
PID TTY TIME CMD AAG
```
`CMD` = short command (`comm`); `-f`/BSD `u` switch it to the full argument vector. `AAG` is the peios at-a-glance security column (§6.5) — the one default addition, dropped under compat.

**Divergence:** procps default is `PID TTY TIME CMD` (four columns). peios appends `AAG`.

### 4.2 Custom columns
`-o <list>` (and `-O`) select columns by keyword; `-o field=Header` renames a header, `-o field=` blanks it (procps-faithful). The full keyword reference is Appendix E.

### 4.3 Format precedence & errors (procps-faithful)
- `-o` **replaces** the default/preset column set entirely; `-O` **appends** its columns (after `PID`).
- Multiple presets (`-f -l`) or repeated `-o`: **last wins**.
- An **unknown `-o` keyword is an error** (non-zero exit; code per §15), not a silent empty column.
- Default row order (no `--sort`) is **ascending PID** (§9).
- A classic (Linux) column whose `/proc/<pid>/{stat,status}` source is unreadable degrades **as procps does** (`-`/`0`/omit per field), *not* the peios `?`; the peios `?` marker is for the security columns (§11).

---

## 5. Identity and SID rendering (D6, D7)

### 5.1 The SID keyword namespace
`sid` means **security id** everywhere — a deliberate divergence from procps (where `sid`/`-s` mean *session*). The **True SID** of a process is its **primary-token user SID** (the identity it runs as), exposed at `/proc/<pid>/sid`; a *thread's effective* SID (which differs from the True SID only while impersonating) is at `task/<tid>/sid` (§8).

| Concept | `-o` keyword | Selector | Renders as |
|---|---|---|---|
| security SID (user, **True**) | `sid` | (display; see `--sid` below) | raw `S-1-…` |
| friendly owner | `user` | `-u` / `-U` | `sid_render` → name-else-SID |
| projected uid | `uid` | (`-u` accepts a uid as *input*) | number — *display* opt-in only (§1.2) |
| session id | `sesid` | `-s` / `--sesid` | number |

> **Scope note (footgun):** the **`sid` column** shows the user/True SID, but the **`--sid` selector** (§3.2) matches a SID *anywhere* in the security context (any class, from `/proc/<pid>/sids`). Same name, different files, different scope — so `ps --sid X -o sid` can legitimately show a `sid` column whose value is **not** `X` (X matched a group/integrity/PIP SID).

`secsid` (an earlier name for the raw security SID) is **dropped**; `sid` is the raw security SID. **Divergence:** `sid`/`-s` no longer mean session; `--sid` selects by security SID, not session. Compat (§12) reverts both — `sid`/`-s` mean session and `--sid` becomes a session selector.

### 5.2 `sid_render` (libpeios — net-new; §13.2)
All SID display goes through one `sid_render` function. **It does not exist yet** — `token/sid_render.rs` is an applet-private starting point (NT-SID table only, no sigil/socket/integrity/PIP tables); v1 promotes and extends it (§13.2). The model:
```
sid_render(sid):
   well-known table → "*System Integrity", "*Administrators", "*Protected-PeiosTcb"   (authoritative built-ins)
   else /run/resolve-sid.sock → "jack"                                                 (directory-resolved, authd)
   else → "S-1-5-21-…"                                                                  (raw fallback)
```
- The leading **`*` sigil** marks authoritative built-in names (anti-spoof). Invariant: the sigil is prependable **only** by libpeios for genuine well-known SIDs; directory-resolved names with a leading `*` are stripped/rejected, or the marker is forgeable.
- Today authd does not exist, so everything falls back to raw SIDs — the same honest-degrade as `ls`/`lsblk`.
- A reverse lookup (`name → SID`/axis) backs the named selectors (`--integrity high`, `--pip-trust PeiosTcb`).

---

## 6. The PSB / security columns

### 6.1 `sid` — user security SID
Raw `S-1-…` from `/proc/<pid>/sid`. The `user` column is its `sid_render`'d form.

### 6.2 `integrity` (alias `label`) (D8)
The integrity-level SID (`S-1-16-…`), rendered via `sid_render` to a friendly name. Practically a closed enum (`untrusted`(0)/`low`(4096)/`medium`(8192)/`high`(12288)/`system`(16384), per the libpeios well-known table); a custom `S-1-16-…` is theoretically possible and falls back to raw. `--integrity <LEVEL>` accepts the names (and a raw `S-1-16-…`).

### 6.3 `pip` (D9, D10)
**Rendered name only** — `sid_render(/proc/<pid>/pip)` → `None` / `*Protected-PeiosTcb`. No raw-SID column: the PIP SID (`S-1-19-{type}-{trust}`) is trivially derivable from the well-known name (unlike user SIDs). PIP is the trust-of-the-binary axis on the PSB, set at exec by signature verification, immutable; the catalogue is Appendix D. Selectors `--pip` / `--pip-type` / `--pip-trust` (§3.2).

### 6.4 `mitigations` (D11)
Space-separated codes from `/proc/<pid>/mitigations` (e.g. `WXP TLP LSV CFIF CFIB PIE`), collapsing to `ALL` when every active bit is set; verbose `-o` expands to full names. Granular `CFIF`/`CFIB` always (the `CFI` alias is never retained); `UI_ACCESS` hidden while reserved. `-` = none set, `?` = unreadable. Catalogue is Appendix C. `--mitigation <code>` filter (§3.2).

### 6.5 `aag` — At-A-Glance (D13)
A compact flag string of **notable deviations from the security baseline** (baseline = `None` PIP + medium integrity + unconfined). A baseline process (all inputs read, nothing notable) renders **blank** — a deliberate exception to §11's `-` marker, because a flags column scans best when quiet rows are empty and notable flags pop; JSON-empty = `""`. **Partial reads:** `aag` renders the flags it *could* compute and appends a trailing **`?`** to signal partial enumeration — e.g. integrity read but `/pip` EACCES on a process you can't PIP-dominate → `I?`; a fully-unreadable `aag` = `?`. It **never silently drops a flag** (that would lie — you couldn't tell "no `t`" from "couldn't read pip"). Each flag is defined explicitly — there is **no** clean "case = severity" rule (`t` is protected/elevated yet lowercase, by author preference):

| Flag | Meaning | Source |
|---|---|---|
| `t` | PIP protected/isolated | PSB (process-level) |
| `T` | PIP TCB trust | PSB (process-level) |
| `i` | notably low / untrusted integrity | token (per-thread in thread mode) |
| `I` | system integrity | token (per-thread in thread mode) |
| `S` | (future) process silo / confinement | per-thread in thread mode |

The synthesis ("what is notable") lives in `ps`, not the kernel. **Thread mode (§8):** only the token/confinement flags (`i`/`I`/`S`) reflect per-thread effective state; the PIP flags (`t`/`T`) are process-level and identical across a process's threads (PIP is impersonation-invisible). Dropped under compat.

### 6.6 Substrate
Every column above reads a per-process procfs text file (§13.1). The gate on each is **kernel policy**, not `ps`'s concern: `ps` reads-or-EACCES and degrades the cell (§11). `pip`/`mitigations` are process-level (PSB, immutable); `sid`/`integrity` are per-thread effective in thread mode. (`/proc/<pid>/sids`, the fifth file, backs the `--sid` *selector* (§3.2), not a §6 column.)

---

## 7. Format presets (D15)

Standard presets are **structurally Linux-identical** (same columns, order, headers). Their identity columns use the `user` field (name-else-SID), never a uid (§1.2). `STAT` stays the Linux process-state code (distinct from `aag`); `F`/`ADDR` degrade per P2.

| Preset | Columns |
|---|---|
| `-f` | `UID PID PPID C STIME TTY TIME CMD` (UID header, `user` content) |
| `-F` | `UID PID PPID C SZ RSS PSR STIME TTY TIME CMD` (the `SZ RSS PSR` triple inserts after `C`, it is **not** appended after `CMD`) |
| `-l` | `F S UID PID PPID C PRI NI ADDR SZ WCHAN TTY TIME CMD` |
| `-j` | `PID PGID SESID TTY TIME CMD` (procps `SID`→`SESID`, §5.1) |
| `u` / `aux` | `USER PID %CPU %MEM VSZ RSS TTY STAT START TIME COMMAND` |
| `--security` / `--psb` | `PID USER PIP INTEGRITY MITIGATIONS AAG` — **peios-native preset** (the `psb`-tool view as a column set) |

---

## 8. Threads (D17)

`-L` (one line per thread; adds `LWP`/`NLWP`), `-T` (adds `SPID`), `H` (threads as processes), `m`/`-m` (threads after their process) — procps-faithful.

**peios behaviour:** in thread mode the **token-borne** fields render **per-thread effective** — `sid` and `integrity` (from the `task/<tid>/{sid,integrity}` mirrors), and consequently the `i`/`I`/`S` portion of `aag` (§6.5) — which surfaces **impersonation** (a thread acting as a different identity than its process's True SID). The **PSB fields** `pip`/`mitigations` and the `t`/`T` portion of `aag` are process-level and identical across threads (PIP is impersonation-invisible). Thread-mode display is **complementary** to the `--impersonating` filter (§3.2), not a replacement: the filter *selects* processes containing an impersonating thread; thread mode *shows which thread, and as whom*. Under `H` (threads-as-processes), `--impersonating` still evaluates at the **real-process** level — a process's threads are kept or dropped together — since "impersonating" is a property of the process's thread set.

---

## 9. Forest, sort, formatting (D18–D20)

- **Forest** — `--forest`/`f` (ASCII process tree on `CMD`), `-H` (hierarchy indent), `-i` ASCII glyph variant. Built on lsblk's tree glyph/`walk` code. With `--sort`, siblings are sorted **within** each subtree (tree built first, then each sibling list ordered) — parent/child adjacency is preserved.
- **Sort** — `--sort`/`k` (procps-faithful pair) with comma-separated keys and `+`/`-` prefixes. **`-x` is kept as a flagged peios/lsblk-consistent alias** (*divergence:* procps has no `-x` sort flag; `k` is its short form). Built on lsblk's `cmp_by_col`. **Default order (no `--sort`): ascending PID** (procps-faithful). Two-axis columns: `pip` sorts by `(type, trust)` tuple, `integrity` by level rank; **`aag` is not sortable** (a synthesized flag string has no meaningful order) — `--sort aag` is an error.
- **Formatting** — `-w`/`w` (wide; **`-ww`** = unlimited width, which wins over `--cols`/`--width`), `--no-headers`, `-o field=Header` rename, `--cols`/`--width N`. Built on lsblk's table + width renderer.

---

## 10. Output modes (D21)

- **Text** (default) — the column table.
- **`--json`** — a `{"processes": [ … ]}` document. **Divergence:** procps has no JSON; added for toolset consistency and machine-readable security data. **Re-implemented**, not reused — lsblk's `render_json` is hardwired to `{"blockdevices":…}` and is only a template. Schema:
  - one object per process; field names are the lowercase `-o` keyword (`pid`, `cmd`, `aag`, …).
  - **types:** JSON **number** for numeric columns (`pid`, `ppid`, `pgid`, `sesid`, `%cpu`, `%mem`, `vsz`, `rss`, `nice`); JSON **string** otherwise; **`null`** for an unreadable field (`?`); empty `aag` = `""`.
  - **threads** (thread mode) nest under a **`"threads"`** array — each thread object carries its per-thread-effective `sid`/`integrity`/`aag` + `lwp`/`spid`. **Forest** children nest under a **`"children"`** array. Both keys may co-occur; forest JSON expresses hierarchy through nesting, so `CMD` is **not** glyph-decorated in JSON.
- **`-P` / `--pairs`** — `KEY="value"` per line. **Divergence** as above; **re-implemented** from lsblk's `render_pairs` template.

Marker conventions carry across modes: `?` (unreadable) → `null` in JSON; `-`/value otherwise (the `aag` exception: empty = `""` in JSON, blank in text).

---

## 11. Degradation and access (P2)

Every cross-process `/proc/<pid>/*` read is gated by the **two-check rule**: the caller needs both the **SD grant** (process right) **and PIP dominance** (`caller.pip_type ≥ target.pip_type` **AND** `caller.pip_trust ≥ target.pip_trust`). **No privilege bypasses PIP** (`SeDebug` clears the SD check, not dominance). So an ordinary `ps` cannot read a TCB process's fields.

`ps` consequences:
- A process **always appears** in the listing — PIDs/dirnames are visible in `getdents` regardless of PIP (v0.20); only per-file reads are gated.
- Each **unreadable field degrades to `?`** (ps-owned marker; the peiosutils-wide "not sure" convention) — never the whole listing. `-`/value distinguishes "read, empty/none." (The one documented exception is `aag`, whose empty state is **blank** and whose *partial* state appends a trailing `?`; §6.5.)
- `ps` is agnostic to *why* a read failed (SD, PIP dominance, hidepid) — it reads-or-EACCES uniformly.

---

## 12. Compat mode (P1)

`PEIOS_COMPAT_MODE=LINUX` — a **peios-wide**, value-based, extensible env convention ("behave as close to Linux as you can"), defined centrally as a platform contract and read by peiosutils via the single render/keyword chokepoint. When `=LINUX`, `ps`:

- renders identity columns as the **projected uid** (not SID/name);
- restores procps meaning to `sid`/`-s` (= **session**): the `sid` keyword and `-s` selector are session, and **`--sid` becomes a session selector**; **all other peios security filters** (`--integrity`/`--pip`/`--pip-type`/`--pip-trust`/`--mitigation`/`--impersonating`, §3.2) are disabled (no procps meaning to restore);
- **drops `AAG`** from the default set (revert to `PID TTY TIME CMD`);
- forces **flat-union** filter semantics (§3);
- suppresses peios-only columns — an explicit `-o pip`/`-o aag`/`-o mitigations`/etc. is omitted, and `-o sid` follows the keyword reversion (renders **session**, per the first bullet); disables registry user-macros (§14).

**Not reverted by compat:** `--json` / `-P` (§10) are peios-wide *additive* output modes — opt-in (a Linux program shelling out to plain `ps` never triggers them) with no procps equivalent to restore, so compat leaves them available. The deferred full-BSD parser (§2) is a *deferral*, not a behaviour swap, so it has no compat action.

---

## 13. Substrate prerequisites (the gating work)

`ps`'s peios surface depends on substrate that **does not yet exist**. Build order is **substrate-first**: the OS author delivers all of §13 (kernel + libpeios) before ps implementation begins. This is that checklist.

### 13.1 procfs files (pkm)
Five per-process text files (+ `task/<tid>/` mirrors for the per-thread ones). Each is a new `seq_file`; mirroring into `task/<tid>/` is a second registration in `tid_base_stuff[]`. The `/proc/<pid>/token` patch is the precedent for the dual registration — but it is an **fd-handler, not a text seq_file**, so it's a *pattern* template, not a drop-in. Gate = kernel policy; `ps` is agnostic (§11).

The read path is **not uniform** — there are two data sources:
- **Token-backed** (`sid`, `integrity`, `sids`, and the `task/<tid>/` mirrors): the data is in the process **token** (`user_sid`, `integrity_rid`, the group/SID set), reachable today via the existing open-process-token path (§13.3). Straightforward.
- **PSB-backed** (`pip`, `mitigations`): **READ-PATH GAP — both.** They are PSB fields, and there is **no PSB getter at any layer** (the only PSB syscall is `set_psb`=1005, set-only; nothing in kernel/ABI/libpeios/peios-rs reads them; the token does *not* carry them). Building these two files needs a **new cross-process PSB getter**. The cited learn spec (`1200.../300-applying-and-lifecycle.md:124-130`) is a *reference, not a drop-in*: it describes a **self-only, token-based** query gated at `PROCESS_QUERY_INFORMATION` + PIP dominance — heavier than the `~QUERY_LIMITED` we want for `/sid`, and a different mechanism from a cross-process procfs file.

| File | Content | Source / status |
|---|---|---|
| `/proc/<pid>/sid` | **True** (primary-token) user SID, canonical `S-1-…` | token; `task/<tid>/sid` = per-thread *effective*; gate ~QUERY_LIMITED |
| `/proc/<pid>/integrity` | integrity-level SID `S-1-16-…` (synthesized from the token's `integrity_rid`) | token; `task/<tid>/integrity` mirror |
| `/proc/<pid>/pip` | PIP trust label `S-1-19-{type}-{trust}` | **PSB — needs new getter** (gap) |
| `/proc/<pid>/mitigations` | space-separated codes | **PSB — needs new getter** (gap) |
| `/proc/<pid>/sids` | labeled `<CLASS> <SID>` per line, full security context | token; powers `--sid` anywhere + `-u`/`-g` class-filtered; kept alongside the individual files (redundancy for the fast single-value path) |

### 13.2 libpeios — `sid_render` (net-new; starting point `token/sid_render.rs`)
`token/sid_render.rs` exists today but is **applet-private** and covers only the 13 NT-style well-known SIDs (Everyone/Administrators/…). v1 **promotes it to libpeios and extends it** — all of the following are unbuilt:
- `sid_render(sid) -> string` (§5.2) + a reverse `name → SID`/axis lookup; canonical OS-wide.
- the **`*` sigil** scheme + the anti-spoof invariant (directory names cannot forge a leading `*`).
- the **integrity** name table (`untrusted/low/medium/high/system`) and the **PIP** label table (`Protected-PeiosTcb` …) that §6.2/§6.3 render from — neither is in the existing table.
- the **`/run/resolve-sid.sock`** resolver protocol, served by **authd** (absent now → raw-SID fallback for arbitrary user SIDs; the well-known tables work offline).

### 13.3 ps userspace
Wire uucore `proc_info.rs` (existing procps walker, currently dead-code — extend with the SID/PSB fields) + the lsblk output engine. Read paths split per §13.1: the **token path** (the `token` tool's open-process-token mechanism) backs `sid`+`integrity`+`sids` only; `pip`+`mitigations` await the new **PSB getter**. **Lazy reads:** a column's procfs file is read only when that column is requested (the default reads only `/sid`+`/integrity`+`/pip` for AAG; `ps aux` etc. read what their preset needs).

---

## 14. Registry user-macros (D16 — banked follow-up, separable)

A **toolset-wide** uucore mechanism (not `ps`-specific, not required for v1): a bareword resolves (last, per §2.1) against `CurrentUser\Applets\Peiosutils\<tool>\QuickDraw\<name>` to a stored flag set. Per-user (no injection), built-ins-win precedence, **disabled under compat**, explicitly **non-portable** (personal convenience, not for scripts). Subsumes the need for hardcoded personal presets.

---

## 15. Deferred / open

- Full dash-sensitive BSD parsing (§2) — the procps personality parser. Curated subset only for v1 (so bare BSD `T` = this-terminal, and the `-T`/`T` dash-collision, are deferred).
- The exact `aag` legend wording, the `--security` preset's final column list, `%CPU`/`%MEM` calculation method, exit codes, and obscure flags (`-c`, `-y`, BSD `k`/`O`/`S`) — to be pinned during implementation.
- Squat-over-time detection (persistent impersonation) — needs temporal sampling or a kernel "impersonating-since" timestamp; likely a monitoring-daemon job, not `ps`. Out of scope.
- `--integrity` is a v1 **filter** (§3.2, AND tier); a column-display shortcut form is not provided in v1.

---

## 16. Audit instructions

Audit on these axes:
1. **Faithfulness** — does every non-divergence reproduce procps `ps(1)` exactly? Flag any silent behaviour change not marked as a divergence.
2. **Divergences** — is each marked divergence (§2 BSD, §3.2 filter AND, §4.1 AAG default, §5.1 sid-meaning, §10 json/pairs, §12 compat) justified and compat-reversible?
3. **Substrate realism** — are §13's file formats, gates, and the mitigations read-path gap accurate against pkm/libpeios? Is anything assumed that doesn't exist?
4. **Degradation** — does §11 hold for every column (no abort, `?` vs `-` distinction, PIP-dominance gating)?
5. **Internal consistency** — keyword namespace (§5.1) vs selectors (§3) vs presets (§7); no `sid`/`sesid` contradictions.

---

## Appendix A — procfs file formats
See §13.1. All text, all `S-1-…` canonical strings (except `/mitigations` = codes and `/sids` = labeled lines).

## Appendix B — AAG legend
`t` PIP protected · `T` PIP TCB · `i` low/untrusted integrity · `I` system integrity · `S` (future) silo. Blank = baseline; `?` = unreadable.

## Appendix C — Mitigation codes (`pkm/uapi/pkm/psb.h`, closed 10-bit set)
`WXP` Write-XOR-Execute · `TLP` Trusted Library Paths (the prefix cache) · `LSV` Library Signature Verification · `CFIF`/`CFIB` forward/backward CFI · `NO_CHILD` forbid new processes · `PIE` require PIE at exec · `SML` Speculation Mitigation Lock · `UI_ACCESS` (reserved) · `ALL` = all bits.

## Appendix D — PIP catalogue (`S-1-19-{type}-{trust}`)
Two sub-authorities: **type** then **trust**. Types: `None`(0), `Protected`(512), `Isolated`(1024). The named trust tiers all live under **type 512 (Protected)** — they are *not* a free-floating axis:

| SID | label | meaning |
|---|---|---|
| `S-1-19-0-0` | `None` | unprotected (default for unsigned) |
| `S-1-19-512-1024` | `Protected-Authenticode` | third-party signed |
| `S-1-19-512-1536` | `Protected-Antimalware` | antimalware |
| `S-1-19-512-2048` | `Protected-App` | Peios-distributed app |
| `S-1-19-512-4096` | `Protected-Peios` | core Peios component |
| `S-1-19-512-8192` | `Protected-PeiosTcb` | Peios TCB |
| `S-1-19-1024-8192` | `Isolated` | reserved (no v0.20 key targets it) |

v0.20: only `None` and `Protected-PeiosTcb` occur in practice (only the TCB key exists). Authoritative source: `learn/.../2200-constants-and-catalogs/200-well-known-sids.md`.

## Appendix E — Column keyword reference (peios additions)
`sid` (raw security SID) · `user` (name-else-SID) · `uid` (projected uid, opt-in) · `sesid` (session) · `integrity`/`label` · `pip` · `mitigations` · `aag`. Classic procps keywords (`pid ppid pgid comm args stat pcpu pmem vsz rss tty time etime nice pri wchan …`) are supported as in procps.
