#!/usr/bin/python3
"""Stage and validate declared Peiosutils artifacts before writing any PPK commit."""

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import TypedDict, cast

sys.dont_write_bytecode = True
from licences import collect  # noqa: E402
from validate import validate  # noqa: E402


class ComponentInput(TypedDict):
    """Declared binary and transitive source inputs from the staging rule."""

    binary: str
    inputs: list[str]


class StageSpec(TypedDict):
    """Internal staging manifest emitted by packages/build.bzl."""

    components: dict[str, ComponentInput]
    sources: list[str]
    version: str
    sdk: str
    rustc: str
    native: bool


LIBDIR = "usr/lib/x86_64-linux-peios"
DESTINATIONS = {
    "peiosutils": "usr/bin/peiosutils",
    "stdbuf": LIBDIR + "/peiosutils/libstdbuf.so",
    "libreg": LIBDIR + "/libreg.so.1",
    "libregman": LIBDIR + "/libregman.so.1",
    "libevman": LIBDIR + "/libevman.so.1",
}


def copy(source: Path, target: Path) -> None:
    """Copy a file while normalizing its mode for an installed payload."""
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)
    target.chmod(0o755 if source.stat().st_mode & 0o111 else 0o644)


def split_debug(component: str, elf: Path, output: Path) -> None:
    """Split ELF debug data and record the GNU build ID and debug link."""
    notes = subprocess.check_output(["readelf", "-n", str(elf)], text=True)
    match = re.search(r"Build ID: ([0-9a-f]+)", notes)
    if not match:
        raise ValueError(f"missing build ID: {elf}")
    build_id = match.group(1)
    debug = output / f"debug/.build-id/{build_id[:2]}/{build_id[2:]}.debug"
    debug.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["objcopy", "--only-keep-debug", elf, debug], check=True)
    subprocess.run(["strip", "--strip-unneeded", elf], check=True)
    subprocess.run(["objcopy", f"--add-gnu-debuglink={debug}", elf], check=True)


def source_files(inputs: list[Path]) -> list[Path]:
    """Resolve declared source files and generated bindings, rejecting missing inputs."""
    files = set()
    for path in inputs:
        if path.is_dir():
            # Generated bindings and build-script source products, not compiled archives.
            files.update(
                p for p in path.rglob("*") if p.is_file() and p.suffix in {".rs", ".h", ".c"}
            )
        elif path.is_file():
            files.add(path)
        else:
            raise ValueError(f"missing declared input: {path}")
    return sorted(files)


def stage_data(
    sources: list[Path], output: Path, version: str, generator: Path, sdk: Path
) -> list[str]:
    """Install every declared applet alias, manual, completion, locale and public header."""
    import gzip

    environment = os.environ | {
        "LD_LIBRARY_PATH": str((sdk / "lib").resolve()),
        "LC_ALL": "C",
        "SOURCE_DATE_EPOCH": "0",
    }
    binary = (output / DESTINATIONS["peiosutils"]).resolve()
    names = subprocess.check_output([binary, "--list"], env=environment, text=True).splitlines()
    if not names or len(names) != len(set(names)):
        raise ValueError("empty or duplicate applet list")
    for name in names:
        if "/" in name or name in {".", "..", "peiosutils"}:
            raise ValueError(f"unsafe applet name: {name}")
        (output / "usr/bin" / name).symlink_to("peiosutils")
    supplements = {
        path.stem: path
        for path in sources
        if path.parent == Path("docs/man") and path.suffix == ".inc"
    }
    commands = [*names, "peiosutils"]
    for name in commands:
        manual = subprocess.check_output([generator, "manpage", name], env=environment)
        if b".TH " not in manual:
            raise ValueError(f"missing manual header: {name}")
        if name in supplements:
            manual += b"\n" + supplements[name].read_bytes()
        path = output / f"usr/share/man/man1/{name}.1.gz"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(gzip.compress(manual, mtime=0))
        if name == "peiosutils":
            continue
        for shell, location in {
            "bash": f"bash-completion/completions/{name}.bash",
            "zsh": f"zsh/site-functions/_{name}",
            "fish": f"fish/vendor_completions.d/{name}.fish",
        }.items():
            completion = subprocess.check_output(
                [generator, "completion", name, shell], env=environment
            )
            if not completion:
                raise ValueError(f"empty {shell} completion: {name}")
            path = output / "usr/share" / location
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(completion)
    for source in sources:
        if source.parent == Path("docs/man") and source.suffix == ".3":
            target = output / "usr/share/man/man3" / (source.name + ".gz")
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(gzip.compress(source.read_bytes(), mtime=0))
        elif source.parts[0] == "include" and source.suffix == ".h":
            copy(source, output / "usr" / source)
        elif (
            source.parent.name == "locales"
            and source.suffix == ".ftl"
            and source.name != "en-US.ftl"
        ):
            program = source.parent.parent.name
            copy(source, output / f"usr/share/locales/{program}/{source.name}")
    library = output / LIBDIR
    for name in ["reg", "regman", "evman"]:
        (library / f"lib{name}.so").symlink_to(f"lib{name}.so.1")
        metadata = library / f"pkgconfig/{name}.pc"
        metadata.parent.mkdir(exist_ok=True)
        metadata.write_text(
            f"prefix=/usr\nlibdir=/{LIBDIR}\nincludedir=${{prefix}}/include\n"
            f"Name: {name}\nDescription: Peios {name} library\nVersion: {version}\n"
            + ("Requires: peios >= 0.5.12\n" if name == "reg" else "")
            + f"Libs: -L${{libdir}} -l{name}\nCflags: -I${{includedir}}\n"
        )
    return names


def check_versions(sources: list[Path], version: str) -> None:
    """Keep Cargo and package versions identical."""
    if tomllib.loads(Path("Cargo.toml").read_text())["workspace"]["package"]["version"] != version:
        raise ValueError("Cargo version differs from Bazel")


def _stage_components(
    components: dict[str, ComponentInput], output: Path, debug_inputs: set[Path]
) -> dict[str, str]:
    if set(components) != set(DESTINATIONS) | {"uudoc"}:
        raise ValueError("incomplete peiosutils binary set")
    expressions = {}
    metadata = [
        p
        for p in debug_inputs
        if p.name in {"Cargo.toml", "Cargo.lock"} or p.parts[0] == "third-party"
    ]
    for name, component in components.items():
        if name != "uudoc":
            elf = output / DESTINATIONS[name]
            copy(Path(component["binary"]), elf)
            split_debug(name, elf, output)
        inputs = source_files([Path(path) for path in component["inputs"]])
        debug_inputs.update(inputs)
        expressions[name] = collect(inputs + metadata, output / "licenses" / name)
    return expressions


def _stage_debug_sources(debug_inputs: set[Path], output: Path, sdk: Path) -> None:
    # Only source inputs declared by Bazel enter debugsource; never walk a checkout or cache.
    debug_root = output / "usr/src/debug/dev.peios.peiosutils"
    for source in sorted(debug_inputs):
        if source.is_relative_to(sdk) or source.suffix in {".so", ".a", ".rlib", ".o"}:
            continue
        if source.is_absolute() or ".." in source.parts:
            raise ValueError(f"non-relative declared source: {source}")
        copy(source, debug_root / source)


def _write_build_record(
    spec: StageSpec, output: Path, debug_inputs: set[Path], expressions: dict[str, str]
) -> None:
    fingerprints = {
        str(path): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(debug_inputs)
        if path.is_file()
    }
    record = {
        "version": spec["version"],
        "userspace_abi": "peios-native" if spec["native"] else "debian-development",
        "release_ready": False,
        "signed": False,
        "checks": "native-hardening-and-payload" if spec["native"] else "development-payload",
        "input_digest": hashlib.sha256(
            json.dumps(fingerprints, sort_keys=True).encode("utf-8")
        ).hexdigest(),
        "licences": expressions,
    }
    (output / "build.json").write_text(
        json.dumps(record, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )


def main() -> None:
    """Stage the declared build inputs and write provenance after validation succeeds."""
    spec = cast(StageSpec, json.loads(Path(sys.argv[1]).read_text(encoding="utf-8")))
    output = Path(sys.argv[2])
    output.mkdir(parents=True, exist_ok=True)
    sources = [Path(path) for path in spec["sources"]]
    check_versions(sources, spec["version"])
    debug_inputs = set(sources)
    expressions = _stage_components(spec["components"], output, debug_inputs)
    _stage_debug_sources(debug_inputs, output, Path(spec["sdk"]))
    expressions["debugsource"] = collect(sorted(debug_inputs), output / "licenses/debugsource")
    names = stage_data(
        sources,
        output,
        spec["version"],
        Path(spec["components"]["uudoc"]["binary"]).resolve(),
        Path(spec["sdk"]),
    )
    os.environ["RUSTC"] = str(Path(spec["rustc"]).absolute())
    validate(
        output, native=spec["native"], sdk=Path(spec["sdk"]), names=names, destinations=DESTINATIONS
    )
    _write_build_record(spec, output, debug_inputs, expressions)


if __name__ == "__main__":
    main()
