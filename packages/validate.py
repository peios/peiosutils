"""Check peiosutils's staged utility and shared-library payloads without privileged operations."""

import gzip
import os
import re
import subprocess
from pathlib import Path

LIBDIR = "usr/lib/x86_64-linux-peios"


def require(condition: bool, message: str) -> None:
    """Keep package gates effective when Python assertions are disabled."""
    if not condition:
        raise ValueError(message)


def readelf(path: Path, option: str) -> str:
    """Read one ELF view, tolerating binary CRC bytes in the debug-link section."""
    return subprocess.check_output(
        ["readelf", option, str(path)],
        text=True,
        errors="replace",
        env=os.environ | {"LC_ALL": "C"},
    )


def build_id(notes: str, path: Path) -> str:
    """Extract the single GNU build identity required by split debugging."""
    matches = re.findall(r"Build ID: ([0-9a-f]+)", notes)
    require(len(matches) == 1 and len(matches[0]) >= 3, f"{path}: missing or ambiguous build ID")
    return matches[0]


def validate_elf(stage: Path, binary: str, destination: str, *, native: bool) -> None:
    """Require linked hardening and the matching shared-family split debug payload."""
    library = binary != "peiosutils"
    elf = stage / destination
    require(elf.is_file() and os.access(elf, os.X_OK), f"{elf}: missing executable")
    header, program, dynamic = (readelf(elf, option) for option in ("-h", "-lW", "-dW"))
    notes, sections = readelf(elf, "-n"), readelf(elf, "-SW")
    require(bool(re.search(r"Type:\s+DYN\b", header)), f"{elf}: not position independent")
    require("GNU_RELRO" in program and "BIND_NOW" in dynamic, f"{elf}: missing full RELRO")
    stack = [line for line in program.splitlines() if "GNU_STACK" in line]
    require(len(stack) == 1 and bool(re.search(r"\sRW\s", stack[0])), f"{elf}: executable stack")
    if not library:
        require(bool(re.search(r"FLAGS_1.*\bPIE\b", dynamic)), f"{elf}: missing PIE flag")
    if binary in {"peiosutils", "libreg"}:
        require("Shared library: [libpeios.so.0]" in dynamic, f"{elf}: missing libpeios linkage")
    if binary == "peiosutils":
        require("Shared library: [libauth.so.1]" in dynamic, f"{elf}: missing libauth linkage")
    if binary.startswith("lib"):
        require(f"Library soname: [{binary}.so.1]" in dynamic, f"{elf}: incorrect client SONAME")
    require(not re.search(r"\((RPATH|RUNPATH|TEXTREL)\)", dynamic), f"{elf}: unsafe dynamic tag")
    require(".debug_info" not in sections, f"{elf}: unsplit debug information")
    if native:
        require("IBT" in notes and "SHSTK" in notes, f"{elf}: missing native CET properties")
        require(bool(re.search(r"\(RELR\)", dynamic)), f"{elf}: missing native packed relocations")
    identity = build_id(notes, elf)
    debug = stage / f"debug/.build-id/{identity[:2]}/{identity[2:]}.debug"
    require(debug.is_file() and debug.stat().st_size > 0, f"{elf}: missing split debug file")
    require(".debug_info" in readelf(debug, "-SW"), f"{debug}: missing debug information")
    require(build_id(readelf(debug, "-n"), debug) == identity, f"{debug}: build ID mismatch")
    require(debug.name in readelf(elf, "--string-dump=.gnu_debuglink"), f"{elf}: wrong debug link")
    forbidden = {str(Path.cwd().resolve()), str(Path.cwd().resolve()).split("/execroot/")[0]}
    for path in (elf, debug):
        contents = path.read_bytes()
        for prefix in forbidden:
            require(len(prefix) <= 1 or prefix.encode() not in contents, f"{path}: leaked {prefix}")


def validate_data(stage: Path, names: list[str]) -> None:
    """Check every generated command artifact and the public development files."""
    for name in [*names, "peiosutils"]:
        with gzip.open(stage / f"usr/share/man/man1/{name}.1.gz", "rt") as stream:
            require(".TH " in stream.read(), f"missing manual: {name}")
        if name == "peiosutils":
            continue
        require((stage / "usr/bin" / name).readlink() == Path("peiosutils"), f"wrong alias: {name}")
        for path in [
            f"bash-completion/completions/{name}.bash",
            f"zsh/site-functions/_{name}",
            f"fish/vendor_completions.d/{name}.fish",
        ]:
            require((stage / "usr/share" / path).stat().st_size > 0, f"missing completion: {path}")
    for name in ["reg", "regman", "evman"]:
        require(
            (stage / LIBDIR / f"lib{name}.so").readlink() == Path(f"lib{name}.so.1"),
            f"wrong linker name: {name}",
        )
        require(
            (stage / f"usr/include/peios/{name}.h").read_bytes()
            == Path(f"include/peios/{name}.h").read_bytes(),
            f"changed header: {name}",
        )


def validate(
    stage: Path, *, native: bool, sdk: Path, names: list[str], destinations: dict[str, str]
) -> None:
    """Validate payloads without live registry mutations or privileged utility operations."""
    require(sdk.is_dir(), "missing declared SDK")
    for binary, destination in destinations.items():
        validate_elf(stage, binary, destination, native=native)
    validate_data(stage, names)
    environment = os.environ | {
        "LD_LIBRARY_PATH": str((stage / LIBDIR).resolve()) + ":" + str((sdk / "lib").resolve())
    }
    result = subprocess.run(
        [stage / "usr/bin/peiosutils", "--list"],
        capture_output=True,
        text=True,
        env=environment,
        check=True,
    )
    require(result.stdout.splitlines() == names, "changed applet list")
