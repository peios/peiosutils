"""Collect notices from the actual declared Bazel crate closure."""

import hashlib
import json
import shutil
import tomllib
from pathlib import Path

NOTICE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE")


def normalise(expression: str) -> str:
    """Normalize whitespace and alternate-license spelling for package metadata."""
    expression = " ".join(expression.replace("/", " OR ").split())
    if "(" not in expression and " AND " not in expression and " WITH " not in expression:
        expression = " OR ".join(sorted(set(expression.split(" OR "))))
    return expression


def combined(expressions: set[str]) -> str:
    """Conjoin distinct crate license expressions without changing their grouping."""
    return " AND ".join(
        f"({value})"
        if len(expressions) > 1
        and any(operator in value for operator in (" OR ", " AND ", " WITH "))
        and not (value.startswith("(") and value.endswith(")"))
        else value
        for value in sorted(expressions)
    )


def supplementary_notices(identity: tuple[str, str], source: str, files: set[Path]) -> list[Path]:
    """Read only declared, version-specific notices with recorded content hashes."""
    manifest = Path("third-party/notices.json")
    if manifest not in files:
        return []
    for record in json.loads(manifest.read_text(encoding="utf-8")):
        if (record["name"], record["version"]) != identity or record["source"] != source:
            continue
        notices = []
        for notice in record["notices"]:
            path = Path(notice["path"])
            if path not in files:
                raise ValueError(f"undeclared supplementary notice: {path}")
            if hashlib.sha256(path.read_bytes()).hexdigest() != notice["sha256"]:
                raise ValueError(f"supplementary notice hash mismatch: {path}")
            notices.append(path)
        return notices
    return []


def collect(inputs: list[Path], output: Path) -> str:
    """Fail on missing metadata/notices; no Cargo invocation or ambient crate cache."""
    files = {path for path in inputs if path.is_file()}
    manifests = sorted(path for path in files if path.name == "Cargo.toml")
    if not manifests:
        raise ValueError("empty crate metadata closure")
    output.mkdir(parents=True, exist_ok=True)
    expressions = {"MIT"}  # Every first-party crate is covered by the repository licence.
    rows = ["crate\tversion\tdeclared-license\tsource\n"]
    seen = set()
    lock = tomllib.loads(Path("Cargo.lock").read_text(encoding="utf-8"))["package"]
    sources = {(p["name"], p["version"]): p.get("source", "workspace/path") for p in lock}
    for manifest in manifests:
        package = tomllib.loads(manifest.read_text(encoding="utf-8")).get("package")
        if not package:
            continue
        # Only repository members may inherit metadata from the declared root manifest.
        if "external" not in manifest.parts:
            workspace = (
                tomllib.loads(Path("Cargo.toml").read_text(encoding="utf-8"))
                .get("workspace", {})
                .get("package", {})
            )
            for key in ("version", "license"):
                if package.get(key) == {"workspace": True}:
                    package[key] = workspace[key]
        identity = (package["name"], package["version"])
        if identity in seen:
            continue
        seen.add(identity)
        if identity not in sources:
            raise ValueError(f"crate absent from Cargo.lock: {identity}")
        source = sources[identity]
        declared = package.get("license")
        if not isinstance(declared, str) or not declared:
            raise ValueError(f"missing licence expression: {manifest}")
        expression = normalise(declared)
        expressions.add(expression)
        rows.append("\t".join((*identity, expression, source)) + "\n")
        if source == "workspace/path":
            continue
        notices = sorted(
            path
            for path in files
            if path.is_relative_to(manifest.parent)
            and path.relative_to(manifest.parent).parts[0].upper().startswith(NOTICE_PREFIXES)
        )
        if not notices:
            for repository, name in [
                ("peios-rs", "peios-rs"),
                ("authd", "authd"),
                ("gxwi", "gxwi"),
                ("peiosutils", "peiosutils"),
            ]:
                if source.startswith("git+https://github.com/peios/" + repository + ".git"):
                    notice = Path("third-party") / (name + "-LICENSE")
                    if notice in files:
                        notices = [notice]
        if not notices:
            notices = supplementary_notices(identity, source, files)
        if not notices:
            raise ValueError(f"missing declared licence notices: {manifest}")
        destination = output / "-".join(identity)
        destination.mkdir()
        for notice in notices:
            relative = (
                notice.relative_to(manifest.parent)
                if notice.is_relative_to(manifest.parent)
                else Path(notice.name)
            )
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(notice, target)
    (output / "crate-licenses.tsv").write_text("".join(rows), encoding="utf-8")
    expression = combined(expressions)
    (output / "crate-expression.txt").write_text(expression + "\n", encoding="utf-8")
    return expression
