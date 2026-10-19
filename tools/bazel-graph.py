#!/usr/bin/env python3
"""Record the Linux Cargo dependency graph used by native Bazel targets.

Cargo resolves metadata here but never compiles a crate. This explicit update
step is separate from ordinary Bazel builds and consumes the checked-in lockfile.
"""

import argparse
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TRIPLE = "x86_64-unknown-linux-gnu"
RUST_VERSION = "1.98.1"
PRODUCTION_PACKAGES = [
    "peiosutils",
    "libreg",
    "libregman",
    "libevman",
    "peios-reg",
    "peios-regman",
    "peios-evman",
]
PRODUCTION_FEATURES = ["feat_os_unix", "feat_external_libstdbuf", "uudoc"]


def cargo(arguments: list[str], *, offline: bool) -> str:
    """Resolve locked metadata with the same Rust release used by the build."""
    command = ["cargo", "+" + RUST_VERSION, *arguments, "--locked"]
    if offline:
        command.append("--offline")
    return subprocess.check_output(command, cwd=ROOT, text=True)


def production_features(*, offline: bool) -> dict[tuple[str, str], set[str]]:
    """Use production dependency edges rather than metadata's test-feature union."""
    arguments = [
        "tree",
        "--target",
        TRIPLE,
        "--no-default-features",
        "--prefix",
        "none",
        "--edges",
        "normal,build",
        "--format",
        "{p}|{f}",
        "--features",
        ",".join("peiosutils/" + feature for feature in PRODUCTION_FEATURES),
    ]
    for name in PRODUCTION_PACKAGES:
        arguments += ["-p", name]
    features = {}
    for line in cargo(arguments, offline=offline).splitlines():
        if not line:
            continue
        match = re.fullmatch(r"([^ ]+) v([^ ]+)(?: [^|]*)?\|(.*?)(?: \(\*\))?", line)
        if match is None:
            raise ValueError(f"unexpected Cargo tree output: {line}")
        name, version, values = match.groups()
        features.setdefault((name, version), set()).update(filter(None, values.split(",")))
    return features


def enabled_dependencies(package: dict, features: list[str]) -> set[str]:
    """Account for implicit optional features and explicit dep:name activation."""
    enabled = set(features)
    for feature in features:
        for value in package["features"].get(feature, []):
            if value.startswith("dep:"):
                enabled.add(value[4:])
            elif "/" in value and "?/" not in value:
                enabled.add(value.split("/")[0])
    return enabled


def label(package: dict) -> str:
    """Map a workspace Cargo library to its native target."""
    path = Path(package["manifest_path"]).parent.relative_to(ROOT).as_posix()
    return "//" + ("" if path == "." else path) + ":library"


def dependency_groups(
    package: dict, node: dict, packages: dict, members: set[str], features: list[str]
) -> tuple[dict, dict]:
    """Keep target-filtered edges, actual crate identities and Rust import aliases."""
    groups = {
        kind: {key: [] for key in ["local", "external", "proc_local", "proc_external"]}
        for kind in ["normal", "dev", "build"]
    }
    aliases = {}
    enabled = enabled_dependencies(package, features)
    for dependency in node["deps"]:
        other = packages[dependency["pkg"]]
        procedural = any("proc-macro" in target["kind"] for target in other["targets"])
        for edge in dependency["dep_kinds"]:
            kind = edge["kind"] or "normal"
            declarations = [
                item
                for item in package["dependencies"]
                if item["name"] == other["name"]
                and (item["kind"] or "normal") == kind
                and (not item["optional"] or (item["rename"] or item["name"]) in enabled)
            ]
            if not declarations:
                continue
            prefix = "proc_" if procedural else ""
            if dependency["pkg"] in members:
                target = label(other)
                groups[kind][prefix + "local"].append(target)
                aliases[target] = dependency["name"]
            else:
                rename = next(
                    (
                        item["rename"]
                        for item in declarations
                        if item["rename"] and item["rename"].replace("-", "_") == dependency["name"]
                    ),
                    None,
                )
                groups[kind][prefix + "external"].append(rename or other["name"])
    for group in groups.values():
        for key, values in group.items():
            group[key] = sorted(set(values))
    return groups, dict(sorted(aliases.items()))


def graph(metadata: dict, features: dict[tuple[str, str], set[str]]) -> list[dict]:
    """Normalize paths and omit the resolution-only feature anchor and benchmarks."""
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    members = set(metadata["workspace_members"])
    result = []
    for identity in members:
        package = packages[identity]
        if package["name"] == "peiosutils-build-features":
            continue
        directory = Path(package["manifest_path"]).parent
        relative = directory.relative_to(ROOT).as_posix()
        selected = sorted(
            features.get((package["name"], package["version"]), set(nodes[identity]["features"]))
        )
        groups, aliases = dependency_groups(package, nodes[identity], packages, members, selected)
        targets = []
        for target in package["targets"]:
            if target["kind"] == ["bench"]:
                continue
            values = {
                key: target[key]
                for key in ["name", "kind", "test", "required-features"]
                if key in target
            }
            values["source"] = str(Path(target["src_path"]).relative_to(directory))
            targets.append(values)
        result.append(
            {
                "path": "" if relative == "." else relative,
                "name": package["name"],
                "version": package["version"],
                "edition": package["edition"],
                "features": selected,
                "deps": groups,
                "aliases": aliases,
                "targets": targets,
            }
        )
    return sorted(result, key=lambda crate: crate["path"])


def main() -> None:
    """Update the reviewed graph, or fail if manifest resolution has changed."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument(
        "--fetch", action="store_true", help="allow Cargo to fetch missing locked sources"
    )
    args = parser.parse_args()
    metadata = json.loads(
        cargo(
            ["metadata", "--format-version=1", "--filter-platform", TRIPLE], offline=not args.fetch
        )
    )
    result = graph(metadata, production_features(offline=not args.fetch))
    path = ROOT / "build/crate-graph.json"
    content = json.dumps(result, indent=4) + "\n"
    if args.check:
        if path.read_text(encoding="utf-8") != content:
            parser.exit(
                1, "Cargo metadata changed; run tools/bazel-graph.py, then tools/bazel-targets.py\n"
            )
    else:
        path.write_text(content, encoding="utf-8")


if __name__ == "__main__":
    main()
