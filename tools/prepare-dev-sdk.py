#!/usr/bin/env python3
"""Build the development C and authentication SDKs from explicit source checkouts."""

import argparse
import hashlib
import json
import shutil
import subprocess
import tomllib
from pathlib import Path


def output(source: Path, command: list[str]) -> str:
    """Read a successful command's standard output without hiding diagnostics."""
    return subprocess.check_output(command, cwd=source, text=True).strip()


def libraries(source: Path, targets: dict[str, str], output_base: Path | None) -> dict[str, Path]:
    """Build with the source repository's pinned Bazel and locate declared outputs."""
    bazel = [str(source / "bazel")]
    if output_base:
        bazel.append(f"--output_base={output_base}")
    subprocess.run([*bazel, "build", *targets.values()], cwd=source, check=True)
    execution_root = Path(output(source, [*bazel, "info", "execution_root"]))
    result = {}
    for name, target in targets.items():
        files = output(source, [*bazel, "cquery", target, "--output=files"]).splitlines()
        if len(files) != 1:
            raise RuntimeError(f"expected one library for {target}, got {files}")
        result[name] = execution_root / files[0]
    return result


def source_record(source: Path) -> dict[str, str]:
    """Record the exact source state without presenting local changes as published."""
    return {
        "commit": output(source, ["git", "rev-parse", "HEAD"]),
        "status": output(source, ["git", "status", "--porcelain"]),
        "diff_sha256": hashlib.sha256(
            subprocess.check_output(["git", "diff", "HEAD"], cwd=source)
        ).hexdigest(),
    }


def pkg_config(destination: Path, name: str, version: str) -> None:
    """Write development metadata for an explicitly prepared SDK root."""
    (destination / f"lib/pkgconfig/{name}.pc").write_text(
        f"prefix={destination}\nlibdir=${{prefix}}/lib\nincludedir=${{prefix}}/include\n"
        f"Name: {name}\nDescription: Peios development {name} SDK\nVersion: {version}\n"
        f"Libs: -L${{libdir}} -l{name}\nCflags: -I${{includedir}}\n"
    )


def prepare(
    source: Path,
    auth: Path,
    destination: Path,
    output_base: Path | None,
    auth_output_base: Path | None,
) -> None:
    """Install matching headers and libraries and record their source provenance."""
    if destination.exists():
        raise ValueError("destination must not exist; choose a fresh SDK directory")
    if any(destination == tree or tree in destination.parents for tree in [source, auth]):
        raise ValueError("put development SDK output outside both source checkouts")
    if output_base is not None and output_base == auth_output_base:
        raise ValueError("each source checkout needs its own Bazel output base")
    built = libraries(
        source,
        {"libpeios.so.0": "//libpeios:shared", "libpeios.a": "//libpeios:static"},
        output_base,
    )
    built.update(libraries(auth, {"libauth.so.1": "//libauth:soname"}, auth_output_base))
    version = tomllib.loads((source / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    auth_version = tomllib.loads((auth / "libauth/Cargo.toml").read_text())["package"]["version"]
    destination.mkdir(parents=True)
    shutil.copytree(source / "libpeios/include", destination / "include")
    shutil.copytree(source / "vendor/kernel/include/uapi/pkm", destination / "include/pkm")
    shutil.copytree(auth / "libauth/include", destination / "include", dirs_exist_ok=True)
    lib = destination / "lib"
    lib.mkdir()
    for name, library in built.items():
        shutil.copyfile(library, lib / name)
    (lib / "libpeios.so").symlink_to("libpeios.so.0")
    (lib / "libauth.so").symlink_to("libauth.so.1")
    (lib / "pkgconfig").mkdir()
    pkg_config(destination, "peios", version)
    pkg_config(destination, "auth", auth_version)
    provenance = {
        "version": version,
        **source_record(source),
        "auth": {"version": auth_version, **source_record(auth)},
        "kernel": json.loads((source / "vendor/kernel/ORIGIN.json").read_text()),
        "sha256": {
            str(path.relative_to(destination)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(destination.rglob("*"))
            if path.is_file()
        },
    }
    (destination / "provenance.json").write_text(json.dumps(provenance, indent=4) + "\n")


def main() -> None:
    """Parse explicit source and output paths for local builds and CI."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--auth-source", type=Path, required=True)
    parser.add_argument("--output-base", type=Path)
    parser.add_argument("--auth-output-base", type=Path)
    args = parser.parse_args()
    prepare(
        args.source.resolve(),
        args.auth_source.resolve(),
        args.destination.resolve(),
        args.output_base.resolve() if args.output_base else None,
        args.auth_output_base.resolve() if args.auth_output_base else None,
    )


if __name__ == "__main__":
    main()
