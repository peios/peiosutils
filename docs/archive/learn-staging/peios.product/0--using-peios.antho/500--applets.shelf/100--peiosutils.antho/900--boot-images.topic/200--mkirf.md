---
title: mkirf
type: reference
description: Compile the /boot/initramfs/ source tree into a deterministic cpio initramfs image, resolving hook order and validating the layout. Has a watch mode.
related:
  - peios/boot-and-trust-establishment/initramfs-stage
  - peios/developing-for-peios/writing-boot-hooks/writing-a-boot-hook
  - peios/boot-and-trust-establishment/overview
---

`mkirf` compiles an initramfs *source tree* into an initramfs *image*. The source tree is an ordinary directory — on a running Peios system that directory is [`/boot/initramfs/`](~peios/boot-and-trust-establishment/initramfs-stage) — whose contents map 1:1 onto `/` inside the initramfs. The image is a single deterministic, compressed newc cpio archive — zstd by default, `--compress gzip` for a kernel built without zstd support — that the bootloader loads into memory alongside the kernel.

```
mkirf [--watch] [--debounce SECS] [--exclude GLOB]... [--compress ALGO] <src-dir> <out-file>
```

This page is the command reference. For what the initramfs stage *is* — prelude, the `/boot/initramfs/` layout, and the handoff to the real root — see [The initramfs stage](~peios/boot-and-trust-establishment/initramfs-stage); for how hooks declare their order, see [Boot hooks](~peios/developing-for-peios/writing-boot-hooks/writing-a-boot-hook). `mkirf` reads `<src-dir>` and writes `<out-file>`; it never writes back into the source tree, so the directory you inspect is always exactly what packages and you have put there.

## What a build does

Given a source tree, one build runs a fixed sequence:

1. **Validate the layout.** `<src-dir>` must be a directory, must carry an executable `init` (the initramfs PID 1 — a symlink is fine as long as it resolves within the tree), and must *not* already contain a hook sequence — `hooks.seq` or `system/prelude/hooks.seq.<n>` — which `mkirf` generates. A missing or non-executable `init`, or a stray sequence file, stops the build.
2. **Walk the tree.** Every object beneath `<src-dir>` becomes a cpio entry — regular files (with their executable bit preserved), directories, symlinks (target stored verbatim), character/block device nodes, and FIFOs. Sockets, and any other unsupported type, are rejected. Entries are sorted in `LC_ALL=C` byte order, which places every directory before its descendants — the order the kernel's unpacker requires.
3. **Resolve the hook order.** Hooks are the regular files directly under `<src-dir>/usr/libexec/prelude/hooks.d/` (packaged) or `<src-dir>/lcl/libexec/prelude/hooks.d/` (operator-placed), with the operator's copy winning when both hold the same file name. The legacy `<src-dir>/hooks/` is still read, and each hook found there is reported with the directory it should move to. `mkirf` parses each hook's `# /// hook` metadata block, topologically sorts the resulting capability DAG, and bakes the execution order into generated manifests injected into the image at `/system/prelude/hooks.seq.<n>`. A tree with none of those three directories, or with nothing in them, simply has no hooks — the manifests are still written, empty, because prelude treats a *missing* sequence as a broken image rather than as "nothing to run".

   The version is in the **file name**, and `mkirf` writes every version it can express faithfully — currently `hooks.seq.1` (the flat resolved order) and `hooks.seq.2` (that order plus each hook's declarations). `mkirf` ships in `peiosutils` and prelude ships in its own package, so an image can pair a newer writer with an older reader; naming the versions separately lets one image satisfy both, which a single file carrying an in-band version cannot. A version is dropped only when a future format carries something it can no longer project honestly.

   They live under `/system` — the tier for *derived* content, alongside `system/retc` — rather than `/usr`, which is the vendor tier no package ships these into, or `/var/state`, which is for mutable state rather than immutable build output. See [Validation and errors](#validation-and-errors) below.
4. **Write atomically.** The archive is written to a sibling temp file and `rename`d into place, so a failed run never leaves a half-written image behind. On success `mkirf` prints the path, entry count, and byte size to stderr.

### Early (pre-decompression) segments

A reserved `<src-dir>/++/` directory, if present, holds *early* initramfs segments — content the kernel consumes **before** it decompresses the main archive, namely CPU microcode and ACPI table overrides. Each immediate child of `++/` is one segment whose own contents map onto the cpio root (`++/microcode/kernel/x86/microcode/GenuineIntel.bin` becomes `kernel/x86/microcode/GenuineIntel.bin`); the segment directory name is a human label and never appears in the archive. Every `++/` entry must itself be a directory. The segments are merged into one **uncompressed** cpio prepended ahead of the compressed main archive. With no `++/` directory the output is exactly a single compressed member. `mkirf` stays format-agnostic here: it knows "early segments", never "microcode".

## The deterministic-build guarantee

The same source tree always compiles to the same image, byte for byte. This is what makes "did anything actually change?" a meaningful question and underpins later work such as signed boot artifacts. Determinism comes from normalising everything that would otherwise vary between builds:

- **Ownership, inode numbers, and timestamps** are all emitted as zero.
- **Permission bits are normalised.** The source tree is authoritative for file *type* and, for regular files, *executability* — nothing else. Read/write permission bits are not Peios's access mechanism, so they are flattened to a constant: directories `0755`, symlinks `0777`, FIFOs and device nodes `0644`, regular files `0755` if executable and `0644` otherwise.
- **The compressed member** uses a fixed compressor at a fixed level: zstd level 3 by default (on a real image it compresses ~9x faster than gzip to a slightly smaller archive, and the kernel decompresses it ~6x faster at boot), or gzip level 6 with mtime 0 and no embedded filename (the `gzip -n` equivalent) under `--compress gzip`.
- **The early region** is byte-stable, with file payloads padded to a 16-byte boundary by widening the preceding entry's name field with NUL bytes.

## Validation and errors

`mkirf` will not produce an image from a source tree that cannot boot. A misconfiguration is caught when the image is built — on a running system where the message is easy to read — rather than as a mystery failure at the next boot. The checks:

- **Layout.** `<src-dir>` is not a directory; no `init`, or `init` is not a regular file, or is not executable; a pre-existing hook sequence; a `++` entry that is not a directory; two `++` segments defining the same file. (Exit 1.)
- **Hooks.** A malformed `# /// hook` metadata block (unclosed, a non-comment line inside it, a duplicate or unknown key, a badly-formed array), a dependency **cycle**, a `requires` that **nothing supplies**, or a capability that is **both provided and contributed to** (a capability is either a set of alternatives or a set of contributors, never both). An `after` naming a capability nothing supplies is *not* an error — that is what distinguishes it from `requires`. (Exit 1.) A hook with no metadata block at all is *not* an error — it is the escape hatch: it is scheduled last, in name order, and `mkirf` prints a warning so a forgotten block is visible.
- **I/O.** Any filesystem read, write, or compression failure. (Exit 1.)
- **Usage.** A semantic invocation error clap cannot express — currently, `--watch` with `<out-file>` inside `<src-dir>` (see below). (Exit 2.)

## Watch mode

With `--watch`, `mkirf` stays resident: it builds once up front (so the watch never begins from a stale archive), then watches `<src-dir>` recursively and rebuilds after every change, debounced. This is what lets `/boot/initramfs/` behave like an ordinary part of the filesystem — edit a hook and the initramfs is current again — rather than a build artifact an administrator has to remember to regenerate.

Watch mode is a foreground loop that runs until killed; supervising and restarting it is a service manager's job, not `mkirf`'s. A rebuild that fails (say, a hook edited into a dependency cycle) is logged but does **not** stop the watch, so fixing the offending file recovers on the next change.

`<out-file>` must not live inside `<src-dir>`: each rebuild would write the image back into the watched tree and retrigger the watch endlessly. `mkirf` refuses this invocation up front (exit 2). The `--debounce` window (default 5 seconds) is the settle time — `mkirf` waits for the tree to be quiet for that long before rebuilding, so a burst of edits produces one rebuild rather than many.

## Options

| Option | Effect |
|---|---|
| `--watch` | Stay resident and rebuild on every change to `<src-dir>`, after an initial build. Runs until killed. |
| `--debounce SECS` | With `--watch`, the settle time before a rebuild. Default `5`. A non-numeric value is a usage error. |
| `--exclude GLOB` | Exclude paths (relative to `<src-dir>`) matching `GLOB` from the image. Repeatable. `*` and `?` stay within a path segment; `**` crosses separators. A matched directory is pruned with its whole subtree. A malformed glob is a usage error. |

The two positionals are both required:

| Positional | Meaning |
|---|---|
| `<src-dir>` | The source tree; its contents map onto `/` in the initramfs. |
| `<out-file>` | The output `cpio.gz` archive. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | The image was built (or `--help`/`--version` was printed). |
| `1` | An operational failure — an invalid layout, an unresolvable hook set, or an I/O/compression error. |
| `2` | A usage error — a bad option, a missing positional, or `--watch` with `<out-file>` inside `<src-dir>`. |

## See also

- [The initramfs stage](~peios/boot-and-trust-establishment/initramfs-stage) — what `mkirf`'s output is for.
- [Boot hooks](~peios/developing-for-peios/writing-boot-hooks/writing-a-boot-hook) — the `# /// hook` metadata block and how order is resolved.
- [mkuki](~peios/boot-images/mkuki) — wraps `mkirf`'s image, together with a kernel and command line, into a bootable UEFI unified kernel image.
