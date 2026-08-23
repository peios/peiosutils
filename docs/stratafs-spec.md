# peiosutils `stratafs` — Inspection Tool Specification

Status: **v1 implemented.**

`stratafs` is the read-only command-line inspector for StrataFS mounts. It
reports the configured stack, explains the resolution and mutation routing of
a path, exposes the kernel's origin answer, and audits a create stratum. It is
shipped as a normal `peiosutils` applet.

The filesystem remains authoritative. The tool reads the current mount table
and `system.stratafs.origin`; it does not maintain a second mount registry.

## 1. Security and consistency rules

- Every command is read-only. In particular, `sweep` reports entries but never
  removes them.
- The tool runs entirely with the caller's authority and does not require,
  acquire, or emulate privilege.
- Direct stratum inspection is still subject to the caller's normal access to
  those paths.
- A directory origin can disclose every participating stratum. If any
  participant cannot be inspected, `resolve` fails rather than returning a
  partial stack.
- Create-stratum recursion is anchored to open directory descriptors, does not
  follow directory-entry symlinks, and checks that a directory was not replaced
  between inspection and open.
- Regular files opened for `diff` use `O_NOFOLLOW` and are read through the
  resulting descriptor. Each side has a 16 MiB limit.
- If a path or its origin changes during resolution in a way the tool can
  detect, the command fails and asks the caller to retry.
- Human path output is byte-preserving through escapes. JSON requires paths to
  be valid UTF-8; it fails instead of emitting a lossy path.

The output predicts StrataFS routing, not KACS authorisation. A reported
`in_place`, `copy_up`, `create`, or `remove` route can still be refused by the
caller's access rights when the operation is attempted.

## 2. Command surface

```text
stratafs list [MOUNT] [--json]
stratafs resolve PATH [--json]
stratafs origin PATH
stratafs sweep [MOUNT] [--json]
stratafs diff PATH
```

Relative arguments are made absolute lexically against the current directory.
The most specific enclosing StrataFS mount is selected for a path. A `MOUNT`
argument must name a StrataFS mount point exactly.

### 2.1 `list`

With no argument, reports every StrataFS mount in the caller's current mount
namespace. No mounts is an empty success. With `MOUNT`, reports only that exact
mount or fails if it is not StrataFS.

Strata are shown highest precedence first with their zero-based index, decoded
path, flags (`create`, `ro`, `am`), and current state:

| State | Meaning |
|---|---|
| `present` | The stratum path exists and is a directory. |
| `absent` | The stratum path does not currently exist. |
| `not_directory` | The configured path exists but is not a directory. |

The mount's generic read-only state is reported separately from per-stratum
`+ro`.

### 2.2 `resolve`

Explains one merged path. For each configured stratum it reports the real
candidate path, object type, flags, and one state:

| State | Meaning |
|---|---|
| `provider` | The highest-precedence object that provides the name. |
| `participant` | A lower directory participating in the merged directory. |
| `shadowed` | A lower object of the provider's type that does not provide the name. |
| `masked` | A lower object hidden by a provider of another type. |
| `absent` | This stratum does not hold the path. |

The kernel's origin xattr is used to validate the provider and the participant
set. The report then predicts two independent actions:

- `write`: `in_place`, `copy_up`, `create`, `follow_symlink`, `erofs`,
  `enoent`, or `unknown`.
- `delete`: `remove`, `erofs`, `enoent`, or `unknown`.

`unknown` means the underlying filesystem did not expose the immutable-attribute
fact needed for a sound routing answer. A directory `remove` report also states
that the merged directory must be empty. When removal exposes a lower object,
the report names the object that will resurface.

Content I/O on a symlink follows the link, so `resolve` reports
`follow_symlink`; routing then belongs to the resolved target. Special-file I/O
is reported `in_place` because it does not mutate the filesystem object itself.

### 2.3 `origin`

Prints the raw value returned by `system.stratafs.origin`, adding a final
newline only when the value lacks one. A non-directory has one provider path. A
merged directory has one escaped participant path per line, in precedence
order. The command is deliberately not available in JSON form because the raw
xattr is the interface.

### 2.4 `sweep`

Recursively inventories every entry in the selected create stratum, including
directories. If there is exactly one StrataFS mount, `MOUNT` may be omitted;
otherwise it is required.

| State | Meaning |
|---|---|
| `gap` | No higher or lower stratum has the relative path. |
| `override` | A lower stratum also has the path. For directories this is structural and the directories may merge. |
| `shadowed` | A higher stratum has the path, or a higher non-directory ancestor makes it unreachable. |

If both a higher and a lower object exist, `shadowed` wins. The related higher
or lower object is included when there is one. An empty create stratum is the
clean/reconciled result.

### 2.5 `diff`

Compares the object in the create stratum with the first lower-precedence object
at the same relative path. Regular files receive a compact unified-style diff;
symlinks compare their link text; differing types receive a type-change report.
Directories and other special objects are not diffable. Binary regular files
are only reported as different, and either regular file exceeding 16 MiB is
refused.

## 3. JSON contract

`--json` is supported by `list`, `resolve`, and `sweep`. It emits one complete
pretty-printed JSON document.

`list` emits an array:

```json
[
  {
    "mount": "/bin",
    "read_only": false,
    "strata": [
      {"index": 0, "path": "/lcl/bin", "flags": ["create"], "state": "present"},
      {"index": 1, "path": "/usr/bin", "flags": ["ro"], "state": "present"}
    ]
  }
]
```

`resolve` emits one object with `path`, `mount`, nullable `object_type`,
`strata`, `write`, and `delete`. Each stratum has `index`, `stratum`, `object`,
`flags`, `state`, and nullable `object_type`. Each action has `action`, nullable
`target`, and `reason`. File types are `regular`, `directory`, `symlink`,
`fifo`, `socket`, `block_device`, `char_device`, or `unknown`.

`sweep` emits an array whose entries have `path`, `state`, `create_object`, and
nullable `related_object`.

Field names and enum spellings above are the stable v1 machine interface.
Human-readable prose is not stable and scripts should use `--json`.

## 4. Exit status

| Status | Meaning |
|---|---|
| `0` | Command succeeded; additionally, `sweep` was empty or `diff` found no difference. |
| `1` | `sweep` found one or more create-stratum entries, or `diff` found a difference. |
| `2` | Usage error, inaccessible/incomplete inspection, malformed mount data, unsupported comparison, or other operational error. |

