---
title: mkuki
type: reference
description: Build a UEFI unified kernel image — kernel, initramfs, and command line appended as PE sections onto an EFI stub. Has a watch mode.
related:
  - peios/boot-and-trust-establishment/initramfs-stage
  - peios/boot-images/mkirf
  - peios/boot-and-trust-establishment/overview
---

`mkuki` builds a **UEFI unified kernel image (UKI)**: it takes a PE/COFF EFI stub and appends the kernel, the initramfs, and the kernel command line to it as named PE sections, producing a single EFI binary that UEFI firmware boots directly. Where [`mkirf`](~peios/boot-images/mkirf) produces the initramfs image, `mkuki` is the step that wraps that image — together with a kernel and a command line — into one bootable artifact. The two are the two halves of Peios' Dynamic Boot system.

```
mkuki (--kernel PATH | --kernel-dir PATH) --initramfs PATH
      (--cmdline TEXT | --cmdline-file PATH) --out PATH
      [--stub PATH] [--watch] [--debounce SECS]
mkuki --stub-info
```

## The UKI section layout

A UKI is an ordinary PE/COFF EFI executable — the *stub* — with three extra sections appended, which the stub reads at boot to find its payloads:

| Section | Content | Source |
|---|---|---|
| `.cmdline` | The kernel command line, NUL-terminated. | `--cmdline` or `--cmdline-file` |
| `.linux` | The kernel image. | `--kernel`, or the unique kernel resolved by `--kernel-dir` |
| `.initrd` | The initramfs cpio image. | `--initramfs` |

`mkuki` appends them in exactly that order. Each new section is marked as initialised, read-only data. The tool works at the PE level directly: it parses the stub's DOS/PE headers and section table, computes correctly-aligned virtual addresses and file offsets for the new sections, appends their data, updates the section count and the `SizeOfImage`/`SizeOfHeaders` fields, and — if the stub's header has no spare room for three more section entries — grows the header region and relocates the existing sections' file pointers to make space. A stub that is not a valid `MZ`/`PE` image, or whose optional header is malformed, is rejected.

### The command line

The command line is taken either literally from `--cmdline` or read from the file named by `--cmdline-file` (exactly one is required). Either way, `mkuki` trims trailing newlines and carriage returns and appends a single NUL terminator. A command line containing an embedded NUL byte is rejected.

### Selecting a release-named kernel

`--kernel` names one concrete image. `--kernel-dir` instead names a directory whose immediate children are kernel releases, each containing a `vmlinuz-*` image—the layout used by `/usr/lib/modules`. Exactly one matching regular file must exist. Zero matches are an incomplete installation, and multiple matches are ambiguous; `mkuki` refuses both rather than guessing which kernel the firmware should boot.

Directory selection matters most in watch mode. Kernel package upgrades replace the release-named child directory, so a watcher bound to the old concrete path would never adopt the new kernel. With `--kernel-dir`, `mkuki` watches the whole directory tree and resolves its unique kernel again before every rebuild. A transient zero- or two-kernel state during an upgrade makes that rebuild fail without killing the watcher; the next filesystem change retries after the package transaction reaches a unique kernel again.

### The stub

`--stub` names the EFI stub to append to. With no `--stub`, `mkuki` uses a stub bundled into the binary (the systemd EFI stub). `mkuki --stub-info` prints that bundled stub's provenance and exits, so you can see exactly what the default is without building anything.

## Output and atomicity

`mkuki` creates any missing parent directories of `--out`, writes the assembled image to a sibling temp file, and `rename`s it into place, so an interrupted or failed build never leaves a half-written UKI behind. On success it prints the output path and byte size to stderr. Each of the three payloads must be non-empty; an empty `.cmdline`, `.linux`, or `.initrd` fails the build.

> [!NOTE]
> `mkuki` does **not** sign the image. Producing the unified binary and signing it for Secure Boot are separate steps; `mkuki` has no signing options. Because a UKI bundles the kernel, initramfs, and command line into one PE image, that image is what a later Secure Boot milestone signs and the firmware verifies as a whole — but the signing itself is out of `mkuki`'s scope.

## Watch mode

With `--watch`, `mkuki` stays resident: it builds once up front, then rebuilds the UKI whenever an input changes — so the boot image tracks a new kernel, a freshly-repacked initramfs (`mkirf`'s half of Dynamic Boot), or an edited command-line file with no manual step. Like [`mkirf`'s watch mode](~peios/boot-images/mkirf), it is a foreground loop that runs until killed; supervising it is a service manager's job, and a rebuild that fails (for example, a kernel caught mid-copy) is logged rather than fatal, so fixing the input recovers on the next change.

The watched inputs are the kernel source, `--initramfs`, and—only if it is a `--cmdline-file`, since a literal `--cmdline` is static—the command-line file. For `--kernel` and the other concrete files, `mkuki` watches each input's **parent directory** non-recursively rather than the file itself. This survives atomic temp-and-rename writes, which appear as directory events where a stale single-file inode watch would miss them. A `--kernel-dir` tree is watched recursively so release directories can be replaced.

An output directly inside a non-recursive watched directory would retrigger the watcher, so `mkuki` refuses it. With a recursive `--kernel-dir`, the output must be outside the entire watched tree for the same reason. The `--debounce` window (default 5 seconds) is the settle time before a rebuild.

## Options

| Option | Effect |
|---|---|
| `--kernel PATH` | A concrete kernel image; becomes the `.linux` section. Mutually exclusive with `--kernel-dir`. |
| `--kernel-dir PATH` | Resolve the unique regular file at `PATH/*/vmlinuz-*`; in watch mode, recursively watch `PATH` and resolve it again for every rebuild. Mutually exclusive with `--kernel`. |
| `--initramfs PATH` | The initramfs cpio; becomes the `.initrd` section. Required (except with `--stub-info`). |
| `--cmdline TEXT` | The kernel command line, given literally; becomes the `.cmdline` section. Mutually exclusive with `--cmdline-file`. |
| `--cmdline-file PATH` | Read the kernel command line from `PATH` instead. Mutually exclusive with `--cmdline`. |
| `--out PATH` | The output UKI path. Required (except with `--stub-info`). |
| `--stub PATH` | The PE/COFF EFI stub to append sections to. Defaults to the bundled systemd stub. |
| `--stub-info` | Print the bundled stub's provenance and exit. |
| `--watch` | Stay resident and rebuild the UKI whenever the selected kernel source, `--initramfs`, or `--cmdline-file` changes. Runs until killed. |
| `--debounce SECS` | With `--watch`, the settle time before a rebuild. Default `5`. |

Exactly one of `--kernel` or `--kernel-dir`, and exactly one of `--cmdline` or `--cmdline-file`, must be given: supplying both alternatives in a pair, or neither, is a usage error.

## Exit status

| Code | Meaning |
|---|---|
| `0` | The UKI was built (or `--stub-info`, `--help`, or `--version` was printed). |
| `1` | A build or watch failure — a missing or unreadable input, an invalid stub, an empty payload, or an I/O error. |
| `2` | A usage error — a bad or missing option, or an invalid `--cmdline`/`--cmdline-file` combination. |

## See also

- [mkirf](~peios/boot-images/mkirf) — builds the `.initrd` payload `mkuki` wraps.
- [The initramfs stage](~peios/boot-and-trust-establishment/initramfs-stage) — what the bundled initramfs does once firmware boots the UKI.
