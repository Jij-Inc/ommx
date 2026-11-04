"""Print the adapter packages that declare support for a CI Python version."""

import sys
import tomllib
from pathlib import Path

FREE_THREADING = "Programming Language :: Python :: Free Threading"


def main():
    target = sys.argv[1]  # The workflow matrix uses e.g. "3.15" or "3.15t".
    minor_classifier = f"Programming Language :: Python :: {target.removesuffix('t')}"
    packages = []
    for path in sorted(Path("python").glob("ommx-*-adapter/pyproject.toml")):
        with path.open("rb") as f:
            metadata = tomllib.load(f)
        classifiers = metadata["project"]["classifiers"]
        if minor_classifier not in classifiers:
            continue
        if target.endswith("t") and not any(
            c == FREE_THREADING or c.startswith(FREE_THREADING + " :: ")
            for c in classifiers
        ):
            continue
        packages.append(path.parent.name)
    print(" ".join(packages))


if __name__ == "__main__":
    main()
