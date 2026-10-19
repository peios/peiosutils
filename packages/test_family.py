#!/usr/bin/env python3
"""Check package boundaries, installable shared libraries and development provenance."""

import json
import tomllib
from pathlib import Path

LIBDIR = "usr/lib/x86_64-linux-peios"
LIBRARIES = ("reg", "regman", "evman")


def require(condition: bool, message: str) -> None:
    """Keep distribution checks effective when Python assertions are disabled."""
    if not condition:
        raise ValueError(message)


def load_family(root: Path) -> dict[str, dict]:
    """Check identities, versions and the complete payload ownership map."""
    index = json.loads((root / "index.json").read_text(encoding="utf-8"))
    expected = {
        "peiosutils",
        "peiosutils-common",
        "peiosutils-debuginfo",
        "peiosutils-debugsource",
        "peiosutils-source",
    }
    expected.update("lib" + name + suffix for name in LIBRARIES for suffix in ("", "-devel"))
    require(index["release_ready"] is False, "incorrect release claim")
    manifests = {}
    ownership = {}
    for package in index["packages"]:
        name = package["name"].removeprefix("dev.peios.")
        require(name not in manifests, "duplicate package identity")
        require(package["version"] == "0.8.18-1", "inconsistent package version")
        manifest = tomllib.loads((root / package["manifest"]).read_text(encoding="utf-8"))
        require(bool(manifest["files"]), "empty package")
        require(package["release_ready"] is False, "package incorrectly marked release-ready")
        if name != "peiosutils-source":
            require(package["build"]["signed"] is False, "incorrect signing claim")
            require(
                package["build"]["userspace_abi"] in {"debian-development", "peios-native"},
                "missing ABI provenance",
            )
        for path, entry in manifest["files"].items():
            if entry.get("dir"):
                continue
            require(path not in ownership, f"overlapping payload: {path}")
            ownership[path] = name
        manifests[name] = manifest["files"]
    require(set(manifests) == expected, "incorrect package family")
    return manifests


def check_utilities(family: dict[str, dict]) -> None:
    """Require every installed alias to have its manual and three completions."""
    runtime, common = family["peiosutils"], family["peiosutils-common"]
    require(runtime["usr/bin/peiosutils"].get("exec") is True, "multicall executable missing")
    require(
        runtime[f"{LIBDIR}/peiosutils/libstdbuf.so"].get("exec") is True, "preload library missing"
    )
    names = {
        Path(path).name
        for path in runtime
        if path.startswith("usr/bin/") and path != "usr/bin/peiosutils"
    }
    require(
        {"cat", "reg", "regman", "evman", "mkuki", "stdbuf"} <= names, "missing required applets"
    )
    for name in names:
        require(
            runtime[f"usr/bin/{name}"] == {"symlink": "peiosutils"}, f"broken applet alias: {name}"
        )
        for path in [
            f"man/man1/{name}.1.gz",
            f"bash-completion/completions/{name}.bash",
            f"zsh/site-functions/_{name}",
            f"fish/vendor_completions.d/{name}.fish",
        ]:
            require("usr/share/" + path in common, f"missing command artifact: {path}")
    require("usr/share/man/man1/peiosutils.1.gz" in common, "missing multicall manual")


def check_libraries(family: dict[str, dict]) -> None:
    """Keep runtime SONAMEs and development link names in their own packages."""
    for name in LIBRARIES:
        runtime, development = family["lib" + name], family["lib" + name + "-devel"]
        require(runtime[f"{LIBDIR}/lib{name}.so.1"].get("exec") is True, f"missing library: {name}")
        require(
            development[f"{LIBDIR}/lib{name}.so"] == {"symlink": f"lib{name}.so.1"},
            f"incorrect linker name: {name}",
        )
        for path in [
            f"usr/include/peios/{name}.h",
            f"{LIBDIR}/pkgconfig/{name}.pc",
            f"usr/share/man/man3/lib{name}.3.gz",
        ]:
            require(
                "blob" in development[path] and "exec" not in development[path],
                f"invalid development file: {path}",
            )


def check_sources(family: dict[str, dict]) -> None:
    """Require the rebuild inputs and one split-debug file per shipped ELF."""
    debug = [path for path in family["peiosutils-debuginfo"] if path.endswith(".debug")]
    require(len(debug) == 5, "incomplete split-debug family")
    source = family["peiosutils-source"]
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "MODULE.bazel",
        "MODULE.bazel.lock",
        ".bazelrc",
        "build/crate-graph.json",
        "tools/bazel-targets.py",
        "tools/bazel-graph.py",
        "include/peios/reg.h",
        "packages/stage.py",
    ]:
        require("usr/src/peiosutils/" + path in source, f"missing rebuild source: {path}")
    require(
        any(
            path.endswith("/registry-manual/src/index.rs")
            for path in family["peiosutils-debugsource"]
        ),
        "missing first-party debug sources",
    )
    require(
        any(
            "/external/" in path and path.endswith(".rs")
            for path in family["peiosutils-debugsource"]
        ),
        "missing dependency debug sources",
    )


def main() -> None:
    """Validate the assembled family without installing or running privileged applets."""
    family = load_family(Path("packages.store"))
    check_utilities(family)
    check_libraries(family)
    check_sources(family)
    print("complete development package family verified")


if __name__ == "__main__":
    main()
