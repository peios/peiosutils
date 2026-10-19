#!/usr/bin/env python3
"""Stage declared locale files in the executable-relative runtime layout."""

import json
import shutil
import sys
from pathlib import Path


def main() -> None:
    """Reject unexpected and colliding source paths before copying translations."""
    sources = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    output = Path(sys.argv[2])
    destinations = set()
    for name in sources:
        source = Path(name)
        if source.parent.name != "locales" or source.suffix != ".ftl":
            raise ValueError(f"unexpected locale input: {source}")
        target = output / source.parent.parent.name / source.name
        if target in destinations:
            raise ValueError(f"duplicate locale input: {target}")
        destinations.add(target)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)


if __name__ == "__main__":
    main()
