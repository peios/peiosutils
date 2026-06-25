# peiosutils `reg` — Implementation Specification

Status: **DRAFT v0.2 — all open questions (O1–O7) resolved; v1 implemented.**

## Implementation status

The `pu_reg` crate (`src/uu/reg/`) is built and wired into the workspace
(`feat_common_core`). `cargo check`/`build -p pu_reg` pass; 10 unit tests for
addressing + the value-literal engine pass.

Implemented commands: `get` `ls` `tree` `info` `set` `new` `del` `mask`
`unmask` `hide` `unhide` `layer {ls,new,set,del}` `sd` `link` `backup`
`restore` `watch` `export` `apply`.

Deferred / partial (tracked):
- **`apply` text input** — JSON apply is exact and complete; the *text* parser
  is held until the §6.1 escaping grammar is finalised (text input returns a
  clear error pointing there). Text **export** works (one-way, for review).
- **`get -L`** — winner + provenance only (spec O7); the full shadowed stack
  awaits the `query_value_layers` ABI.
- **`watch`** — arms `notify` and streams one event per change (drains but does
  not decode record contents; the on-wire record layout is out of scope here).
- **`reg run` private-layer wrapper** — out of v1 (O4).

Offline build note: `peios-sys`'s bindgen needs `LIBCLANG_PATH` and, on hosts
without a clang resource dir, `BINDGEN_EXTRA_CLANG_ARGS="-isystem <gcc-include>"`
so `stddef.h` resolves; run the binary with `LD_LIBRARY_PATH=deps/lib`.

`reg` is the command-line tool for inspecting and manipulating the Peios
registry (LCS — the Layered Configuration Subsystem of the PKM). It is the
live-registry counterpart to `regman`, which only reads on-disk documentation
fragments and never touches the kernel.

This spec defines the command surface, addressing grammar, value-literal
syntax, layer semantics, security integration, and error model. It is written
to be argued with: open questions are collected in §13.

---

## 1. Design principles

1. **The layer model is first-class, not hidden.** LCS resolves every read
   across precedence-ordered layers and tags every write with a layer. `reg`
   exposes this directly rather than pretending the registry is flat. But the
   *common* case — read the effective value, write to the base layer — stays
   one short command.
2. **No privilege gating in the tool.** Following the peios convention
   (`peios-applets-span-privilege`), `reg` never checks the caller's identity
   or refuses an operation pre-emptively. It attempts the op and reports
   whatever KACS returns (EACCES/EPERM → exit 3). A single `reg` command may
   span privilege tiers.
3. **Correct or honest, never weird (`peiosutils-not-source-of-weirdness`).**
   `reg` does not paper over kernel behaviour with guesses. If a value's type
   is ambiguous, the user can be explicit; the tool never silently corrupts a
   type on read-back.
4. **Atomic where the kernel is atomic.** Single mutations auto-commit. Multi-
   operation batches (`reg apply`) run inside one hive-scoped transaction and
   either all land or none do.
5. **Reuse, don't reinvent.** Security descriptors go through the same SDDL
   codec the `sd` tool uses; we do not grow a second SD surface.

---

## 2. Addressing

### 2.1 Key paths

A key is named by a positional path argument. Both `/` and `\` are accepted as
separators on input (neither character is legal inside an LCS key component, so
there is no ambiguity). Examples:

```
reg get Machine/System/Registry
reg get 'Machine\System\Registry'      # identical
```

Canonical display uses backslash by default (matching PSD-005 and `regman`
cross-references). A `--sep=/` flag (or `REG_SEP=/` env) switches display to
forward slash per-invocation. **[O1: decided.]**

Hive roots:

| Path                  | Meaning                                                        |
|-----------------------|---------------------------------------------------------------|
| `Machine\…`           | The machine hive.                                             |
| `Users\<SID>\…`       | A specific user's hive.                                       |
| `CurrentUser\…`       | Kernel alias, rewritten to `Users\<caller-SID>\` at syscall boundary. |

Leading separator is optional and ignored (`/Machine/X` == `Machine/X`).

### 2.2 Value names

The value name is a **separate** positional argument, never folded into the key
path. This avoids the ambiguity that LCS value names may themselves contain `/`
and `\` (key components may not).

```
reg get Machine/System/Registry CurrentVersion
#       └── key path ──────────┘ └─ value name ─┘
```

- **No value argument ⇒ the command targets the key itself** (its metadata,
  its SD, its children).
- **A value argument ⇒ the command targets that named value.**
- The **default value** (the empty-name value) is addressed by the literal
  token `@` in the value-name position, mirroring `.reg` convention:
  `reg get Machine/App @`.

This positional split applies uniformly to `get`, `set`, `del`, `mask`, etc.

---

## 3. Value literals (writes)

On `set`, the value data is a single positional token whose type is **inferred**
unless a `type:` prefix forces it.

### 3.1 Explicit (`type:` prefix)

A token is typed when the substring before its first `:` is a recognised type
keyword:

| Prefix       | Reg type        | Data syntax                                              |
|--------------|-----------------|---------------------------------------------------------|
| `sz:`        | REG_SZ          | UTF-8 string (rest of token, verbatim).                  |
| `expand:`    | REG_EXPAND_SZ   | UTF-8 string with `%VAR%` left unexpanded on disk.       |
| `dword:`     | REG_DWORD       | `42` or `0x2A`; must fit u32.                            |
| `qword:`     | REG_QWORD       | `42` or `0x2A`; must fit u64.                            |
| `multi:`     | REG_MULTI_SZ    | Comma-separated; `\,` escapes a literal comma.          |
| `hex:`/`bin:`| REG_BINARY      | Hex bytes, optional `:`/space/`-` separators ignored.    |
| `link:`      | REG_LINK        | Absolute key path (symlink target value).                |
| `none:`      | REG_NONE        | No data (token must be empty after prefix).              |
| `dword-be:`  | REG_DWORD_BIG_ENDIAN | `42` or `0x2A`; must fit u32.                       |

To force a string that *looks* like a typed token, prefix it: `sz:dword:42`
stores the literal string `dword:42`. A token whose pre-`:` part is **not** a
known keyword (e.g. `http://host`) is never treated as typed.

### 3.2 Inference (no recognised prefix)

| Token shape                              | Inferred type |
|------------------------------------------|---------------|
| all decimal digits, fits u32             | REG_DWORD     |
| all decimal digits, fits u64 (not u32)   | REG_QWORD     |
| `0x…` hex, ≤ 8 sig. hex digits           | REG_DWORD     |
| `0x…` hex, ≤ 16 sig. hex digits          | REG_QWORD     |
| anything else (incl. empty)              | REG_SZ        |

Inference is **broad [O5: decided]** — the §3.2 table is the rule: any
all-digit token becomes DWORD/QWORD (by magnitude), `0x…` becomes DWORD/QWORD
(by width), leading zeros included. The known footgun is accepted: a
numeric-looking string (a PIN, a zip code, `007`) becomes a number and loses
its textual form. **To force a string, write `sz:…`.** The mitigation is that
`reg set` **always echoes the resolved type on success** (even under `-q` the
type is shown when inference produced a numeric type from a leading-zero or
hex token — the surprising cases), so a coercion is never silent:

```
$ reg set Machine/App Build 4096
set Machine\App  Build = REG_DWORD 4096   (layer: base)
```

---

## 4. Command surface

Global flags (accepted by every subcommand): `--json`, `-v/--verbose`,
`-q/--quiet`, `--help`, `--version`.

### 4.0 Environment-variable equivalents

Every behavioural toggle has both a flag and an environment variable, so the
behaviour can be set per-invocation (flag) or once per shell/script (env). The
flag always wins over the env var; the env var wins over the built-in default.

| Toggle | Flag | Env var | Default |
|--------|------|---------|---------|
| Display separator | `--sep=\|/` | `REG_SEP` | `\` |
| Skip confirmations | `-y/--yes` | `REG_ASSUME_YES=1` | prompt on TTY |
| Structured output | `--json` | `REG_JSON=1` | human |
| Verbosity | `-v/--verbose` | `REG_VERBOSE=1` | off |
| Default write layer | `--layer NAME` | `REG_LAYER` | `base` |

(`REG_LAYER` lets an admin pin a working layer for a whole session without
repeating `--layer` on every mutation.)

### 4.1 Read

| Command | Purpose |
|---------|---------|
| `reg get <key> [value] [-L] [--raw] [--no-follow]` | Read the effective (resolved) value. `-L/--layers` annotates it with the winning layer and sequence (full shadowed stack pending an ABI — see §12 / O7). `--raw` writes raw bytes to stdout (for BINARY piping). With no `value`, prints the key's effective values (shorthand for `ls --values-only`); the default (`@`) value is always listed first. |
| `reg ls <key> [-l] [--keys-only] [--values-only]` | One-level listing of subkeys and values. `-l` long form: types, sizes, last-write times. |
| `reg tree <key> [--depth N] [--values]` | Recursive subkey tree; `--values` includes values at each node. |
| `reg info <key> [--no-follow]` | Key metadata: leaf name, subkey/value counts, last-write time, hive generation, SD size, volatile & symlink flags. |

### 4.2 Write

| Command | Purpose |
|---------|---------|
| `reg set <key> <value> <data> [--layer NAME] [-p] [--expected-seq N]` | Create/update a value. Default layer `base`. `-p/--parents` creates missing ancestor keys. `--expected-seq` is an optimistic-concurrency CAS guard (mismatch → exit on EAGAIN). |
| `reg new <key> [--layer NAME] [-p] [--volatile]` | Create a key with no values. `--volatile` makes it RAM-only (children must also be volatile). |
| `reg del <key> [value] [--layer NAME] [-r]` | Delete a value (if `value` given) or a key. Value deletion removes *this layer's* entry only (lower layers resurface). Key deletion requires the key be empty unless `-r/--recursive` (the tool walks and deletes children). |

### 4.3 Layer masking & key visibility

These expose LCS's tombstone / hide primitives, which are distinct from
deletion (they *mask* lower-precedence state rather than removing a layer's own
entry).

| Command | Purpose |
|---------|---------|
| `reg mask <key> <value> --layer NAME` | Per-value tombstone: hides `value` from all layers below `NAME`. |
| `reg mask <key> --all --layer NAME` | Blanket tombstone: hides *all* lower-layer values on the key. |
| `reg unmask <key> [value\|--all] --layer NAME` | Clears the tombstone/blanket. |
| `reg hide <key> --layer NAME` | Creates a HIDDEN path entry, masking the key in `NAME` and below. |
| `reg unhide <key> --layer NAME` | Clears the hide. |

### 4.4 Layers

| Command | Purpose |
|---------|---------|
| `reg layer ls [-l]` | List layers: name, precedence, enabled, owner SID. |
| `reg layer new <name> [--precedence N] [--owner SID] [--disabled]` | Create a layer (writes its metadata key under `Machine\System\Registry\Layers\`). Precedence > 0 requires `SeTcbPrivilege` — the tool attempts and surfaces EPERM. |
| `reg layer set <name> [--precedence N] [--enable\|--disable] [--owner SID]` | Modify layer metadata. |
| `reg layer del <name>` | Delete a layer (removes its metadata key; LCS tears down its entries). |

Per-thread *private* layer attachment (and private-hive scopes) affect only the
calling thread's credentials and can't be done by a short-lived CLI without an
exec-wrapper. **Deferred from v1 [O4: decided]** — v1 manages layer *content*
fully; per-process private *views* (a future `reg run --attach-layer/--scope --
<cmd>`) wait for a concrete consumer and a verified credential-attach ABI.

### 4.5 Security

Reuses the SDDL codec shared with the `sd` tool.

| Command | Purpose |
|---------|---------|
| `reg sd <key> [--owner\|--group\|--dacl\|--sacl]` | Print the key's security descriptor as SDDL. Scope flags limit which components are read (default: owner+group+DACL; SACL needs `ACCESS_SYSTEM_SECURITY`). |
| `reg sd <key> --set <SDDL> [--owner\|--group\|--dacl\|--sacl]` | Apply SD components. Affects future opens only (existing fds keep their granted mask). |

### 4.6 Symlinks

| Command | Purpose |
|---------|---------|
| `reg link <key> <target>` | Create a symlink key pointing at absolute `target`. Requires `KEY_CREATE_LINK` + `SeTcbPrivilege`/Administrator. |

`get`/`info` follow symlinks to the target by default; `--no-follow`
(REG_OPEN_LINK) inspects the link key itself.

### 4.7 Batch, export, backup

| Command | Purpose |
|---------|---------|
| `reg apply <file>` | Apply a batch of operations atomically in **one hive-scoped transaction** (all-or-nothing). Input is the text format in §6. `-` reads stdin. All ops must target one hive (else EXDEV). |
| `reg export <key> <file>` | Dump a subtree to the §6 text format (human-readable, diffable, re-appliable). `--layer NAME` exports one layer's view; default exports effective state. |
| `reg backup <key> <file>` | Kernel binary snapshot of key + subtree (`SeBackupPrivilege`). Opaque, exact, fast. |
| `reg restore <key> <file>` | Replace key + subtree from a backup, in one transaction (`SeRestorePrivilege`). |

`export`/`apply` are the *portable, reviewable* path; `backup`/`restore` are
the *exact, privileged* path.

### 4.8 Watch

| Command | Purpose |
|---------|---------|
| `reg watch <key> [--subtree] [--filter value,subkey,sd] [--count N]` | Arm a change watch and stream records until interrupted (or `--count N` events). `--json` emits one JSON object per line. On watch overflow, emits a synthetic `OVERFLOW` record signalling the watcher should re-read. |

---

## 5. Output

Default output is human-readable, terse, and grep-friendly. `--json` switches
every command to structured output (objects/arrays, one self-contained document
except `watch`, which is JSON-lines).

**Value ordering.** In any value listing (`get <key>`, `ls`), the default
(`@`) value is always emitted first, then named values (case-insensitive sort).
This mirrors regedit's `(Default)`-at-top convention.

Type formatting on read:

| Reg type        | Human form                        | `--json` form                       |
|-----------------|-----------------------------------|-------------------------------------|
| REG_SZ/EXPAND_SZ| the string                        | `{"type":"sz","data":"…"}`          |
| REG_DWORD/QWORD | decimal (`-l`/`--hex` adds hex)   | `{"type":"dword","data":42}`        |
| REG_MULTI_SZ    | one line per element              | `{"type":"multi","data":["a","b"]}` |
| REG_BINARY      | hex, wrapped                      | `{"type":"binary","data":"deadbeef"}` |
| REG_LINK        | target path                       | `{"type":"link","data":"…"}`        |

`-L/--layers` annotates the effective value with the layer it resolved from and
its sequence number **[O7: decided — v1 shows winner + provenance]**:

```
$ reg get Machine\App Theme -L
Theme = REG_SZ "dark"   (layer: policy, seq 7)
```

The full shadowed stack (the `base`/`light` entry beneath the winner) is not
exposed by any current client ABI. When a `query_value_layers` primitive lands
(§12 / O7 follow-up), the **same `-L` flag** upgrades to render the whole stack
with no CLI change:

```
# future, once the ABI exists — same flag, richer output
  policy   (prec 100, seq 7)  REG_SZ "dark"   ← effective
  base     (prec 0,   seq 3)  REG_SZ "light"
```

---

## 6. Batch / export text format

A line-oriented, `.reg`-inspired but Peios-native format. One operation per
stanza; comments with `#`. Draft grammar (see §13 O3):

```
# layer defaults to base unless [layer: NAME] precedes a block
[key Machine\App]
  Build        = dword:4096
  Theme        = sz:dark
  Servers      = multi:alpha,beta
  Legacy       = <delete>            # delete this layer's entry
  Hidden       = <tombstone>         # per-value tombstone

[layer policy]
[key Machine\App]
  Theme        = sz:corporate

[delete-key Machine\App\Temp]        # recursive delete
```

`reg export` emits this; `reg apply` consumes it. The whole file applies as one
transaction per hive.

**Both formats are supported [O3: decided].** JSON is the canonical, exact,
serde-backed representation (`export --json`, machine-generated); the text form
above is the human-facing default for `export` and is git-diffable. `apply`
auto-detects (leading `{`/`[` ⇒ JSON, else text). Because the text grammar is
the tool's highest-risk parser, it carries a **normative escaping section**
(below) that must be airtight before the parser is written.

### 6.1 Text-format escaping (normative — to finalise before build)

- **Value names** are written verbatim up to the first unescaped ` = `. A name
  containing `=`, leading/trailing space, `#`, or a newline must be quoted:
  `"odd = name" = sz:x`. Inside quotes, `\"`→`"`, `\\`→`\`, `\n`→newline.
- **The default value** is the bare token `@` in the name position.
- **`sz:`/`expand:` data**: rest of line verbatim; to include a trailing
  comment-looking `#` or leading/trailing whitespace, quote the whole datum.
- **`multi:`**: comma-separated; `\,`→literal comma, `\\`→`\`.
- **`hex:`/`bin:`**: hex pairs, `:`/`-`/space separators ignored; line
  continuation with trailing `\` for long blobs.
- **Markers**: `<delete>`, `<tombstone>`, `<blanket-tombstone>`, `<hide>` are
  reserved datum tokens; a literal string equal to one must be written `sz:…`.
- JSON has none of these concerns (strings carry bytes directly; binary is a
  hex string field), which is why it is the canonical exact format.

---

## 7. Layer semantics (summary of LCS rules the tool must honour)

- Reads resolve to the **highest-precedence active** entry; ties broken by
  highest sequence number. `get` shows this winner; `-L` shows the full stack.
- Writes are tagged with `--layer` (default `base`). `del <value>` removes only
  the named layer's entry — it is *not* a tombstone.
- A **tombstone** (`reg mask`) actively hides lower layers; a **blanket
  tombstone** (`--all`) hides every lower value on a key.
- Key **hiding** masks a key's existence per layer; removing the layer (or
  `unhide`) brings it back.
- **SD changes are not layered** and are permanent — they mutate the key
  object directly and are not reverted by layer deletion. `reg sd --set` warns
  about this in interactive use.
- **Volatile/symlink/GUID** are per-key immutable properties, not layered.

---

## 8. Transactions & concurrency

- Single mutations auto-commit (the underlying call is given `txn_fd = -1`).
- `reg apply`/`reg restore` open one transaction, enlist every op, then commit;
  any failure aborts the whole batch. Hive-scoped: mixing hives → exit 4 (EXDEV).
- `reg set --expected-seq N` performs a compare-and-swap; a mismatch surfaces
  as exit code for EAGAIN with a clear "value changed under you" message.
- Transaction timeout (default 30 s, kernel-side) on a long `apply` surfaces as
  a distinct error, not a generic failure.

---

## 9. Error model & exit codes

Standard peiosutils convention, extended for registry-specific conditions:

| Exit | Meaning                          | Typical errno |
|------|----------------------------------|---------------|
| 0    | Success                          | —             |
| 1    | Usage error                      | —             |
| 2    | Key/value not found              | ENOENT        |
| 3    | Access denied                    | EACCES, EPERM |
| 4    | Invalid spec (bad path/type/literal, EXDEV, ENAMETOOLONG) | EINVAL, EXDEV |
| 5    | Syscall / source failure         | EIO, ETIMEDOUT, ENOSPC, ENOMEM |
| 6    | CAS conflict (`--expected-seq`)  | EAGAIN        |

Denials report the operation, target, and the access mask that was required, in
the style of the `token`/`sd` tools.

---

## 10. Privilege & honesty notes

- `reg` performs no `getuid`/membership checks. Operations that the spec marks
  as privileged (`layer new --precedence >0`, `link`, `backup`, `restore`,
  SACL access) are simply attempted; the kernel's EPERM is reported faithfully.
- Where LCS itself is immature (e.g. no production source populator yet,
  `kacs-tlp-prefix-no-prod-populator`), `reg` must surface the real kernel
  result rather than fabricating success. "Weird" is acceptable only when it
  traces to the substrate, never when `reg` causes it.

---

## 11. Crate layout (peiosutils convention)

```
src/uu/reg/
  Cargo.toml            # package pu_reg, [[bin]] name = "reg"
  src/
    main.rs             # uucore::bin!(pu_reg);
    reg.rs              # #[uucore::main] uumain + uu_app; library entry
    cli.rs              # clap Command tree, stable arg-id modules
    error.rs            # Error enum -> exit_code()
    addr.rs             # key-path + value-name parsing/canonicalisation
    literal.rs          # value-literal parse (type: prefix + inference) & format
    cmd/
      mod.rs            # dispatch(&ArgMatches)
      get.rs ls.rs tree.rs info.rs
      set.rs new.rs del.rs
      mask.rs hide.rs
      layer.rs sd.rs link.rs
      apply.rs export.rs backup.rs restore.rs
      watch.rs
    reg/                # syscall-orchestration core, clap-free (see below)
      key.rs value.rs txn.rs security.rs
```

The syscall/orchestration core (`reg/`) is kept free of `clap`/`uucore`, mirror-
ing `mount`'s split, so a non-CLI consumer (or a C ABI front) could reuse it.

Registered in the workspace `Cargo.toml` members, as an optional `pu_reg`
dependency, and in the appropriate feature tier.

---

## 12. Implementation binding (confirmed)

The pinned `peios v0.2.0` crate (already used by `mount`/`sd`/`token`) **fully
wraps the registry** — no `peios-sys` FFI shim is required. Mapping of this
spec's commands onto the safe API (`peios::registry`):

| `reg` surface            | peios crate API |
|--------------------------|-----------------|
| open / create key        | `Key::open(parent, path, KeyAccess, OpenFlags)` · `Key::create(…, layer, txn) -> (Key, Disposition)` |
| `get`                    | `key.query_value(name, txn) -> RegValue { sequence, ty, data, layer }` |
| `get -L`                 | winner's `RegValue.layer` + `.sequence` (full stack deferred — O7) |
| `ls` / `tree`            | `key.values(txn)` · `key.subkeys(txn)` iterators; `key.info() -> KeyInfo` |
| `info`                   | `key.info()` (`KeyInfo`: counts, times, `hive_generation`, `sd_size`, `volatile`, `symlink`) |
| `set`                    | `key.set_value(name, ty, data).layer(n).expect_seq(s).in_txn(t).call()` |
| `new`                    | `Key::create(…, CreateFlags::VOLATILE?, layer, txn)` |
| `del <value>`            | `key.delete_value(name, layer, txn)` |
| `del <key>`              | `key.delete_key(layer, txn)` |
| `mask <value>` / `unmask`| `key.set_value(name, ValueType::TOMBSTONE, …)` / `delete_value` |
| `mask --all` / `unmask --all` | `key.blanket_tombstone(layer, set, txn)` |
| `hide` / `unhide`        | `key.hide_key(layer, txn)` / clear via layer path entry |
| `sd` / `sd --set`        | `key.get_security(SecInfo)` + `sddl::format` · `sddl::parse` + `key.set_security(SecInfo, sd, txn)` |
| `link`                   | `Key::create(…, CreateFlags::CREATE_LINK, …)` + default `REG_LINK` value |
| `backup` / `restore`     | `key.backup(fd)` / `key.restore(fd)` |
| `watch`                  | `key.notify(NotifyFilter, subtree)` then poll the key fd |
| `apply` / batch          | `Transaction::begin()` → enlist via `.in_txn(&txn)` → `txn.commit()` |

Types we surface directly: `ValueType` (NONE/SZ/EXPAND_SZ/BINARY/DWORD/
DWORD_BIG_ENDIAN/LINK/MULTI_SZ/QWORD/TOMBSTONE + `from_raw`), `KeyAccess`
bitflags, `SecInfo` (OWNER/GROUP/DACL/SACL/LABEL), `Disposition`,
`TxnState`/`TxnStatus`. SDDL via `peios::security::sddl::{parse, format}` —
identical to the `sd` tool, so `reg sd` output is consistent with `sd`.

> **O7 resolution [decided]:** `query_value` returns only the *effective*
> `RegValue` (carrying the winning `sequence` + `layer`). No client ABI exposes
> the shadowed entries, and they can't be safely reconstructed client-side.
> v1 therefore ships `-L` as **winner + provenance** (the layer that won + its
> sequence). A `query_value_layers` ABI (pkm LCS → libpeios → peios-rs) is
> filed as a tracked follow-up; when it lands, the same `-L` flag upgrades to
> the full shadowed stack with no CLI change.

---

## 13. Open questions (to converge before coding)

- **O1 — Canonical separator. [DECIDED]** Input accepts `/` and `\`. Display
  defaults to `\` (PSD-005/`regman` faithful); `--sep=/` flag and `REG_SEP`
  env switch display to forward slash per-invocation.
- **O2 — `get <key>` with no value. [DECIDED]** Lists the key's effective
  values (shorthand for `ls --values-only`); the default (`@`) value is always
  rendered first, then named values sorted case-insensitively.
- **O3 — Batch format. [DECIDED]** Both. JSON is the canonical exact format;
  the §6 text format is the human-facing `export` default; `apply` auto-detects.
  Text escaping is fully specified in §6.1 (must be finalised before the parser
  is written).
- **O4 — Private layers/hives. [DECIDED: deferred]** Per-process private views
  (the `reg run --attach-layer/--scope -- <cmd>` exec-wrapper) are out of v1.
  v1 manages layer content fully. Revisit with a concrete consumer + verified
  credential-attach ABI.
- **O5 — Inference aggressiveness. [DECIDED: broad]** Any all-digit token →
  DWORD/QWORD by magnitude; `0x…` → DWORD/QWORD by width; leading zeros
  included (`007`→7). Footgun accepted; mitigated by `set` always echoing the
  resolved type (esp. for leading-zero/hex coercions, even under `-q`).
- **O6 — Destructive-op confirmation. [DECIDED]** `del -r`, `restore`,
  `layer del` prompt when stdin is a TTY; auto-skip when non-interactive.
  Skippable via `-y/--yes` **or** `REG_ASSUME_YES=1`. Generalised: all
  behavioural toggles get an env-var equivalent (§4.0).
- **O7 — `get -L` per-layer stack. [DECIDED]** v1 ships `-L` as winner +
  provenance (winning layer + sequence from `RegValue`); the shadowed stack is
  not reconstructable client-side. A `query_value_layers` ABI (pkm LCS →
  libpeios → peios-rs) is a tracked follow-up; the same `-L` flag upgrades to
  the full stack when it lands, with no CLI change.
```
