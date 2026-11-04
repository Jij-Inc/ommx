"""Print the adapter packages that declare support for a CI Python version."""

import sys
import tomllib
from pathlib import Path


def main():
    target = sys.argv[1]  # The workflow matrix uses e.g. "3.15" or "3.15t".
    packages = []
    for path in sorted(Path("python").glob("ommx-*-adapter/pyproject.toml")):
        with path.open("rb") as f:
            metadata = tomllib.load(f)
        if target.endswith("t"):
            ci = metadata.get("tool", {}).get("ommx", {}).get("ci", {})
            supported = target.removesuffix("t") in ci.get("free-threaded", [])
        else:
            classifier = f"Programming Language :: Python :: {target}"
            supported = classifier in metadata["project"]["classifiers"]
        if supported:
            packages.append(path.parent.name)
    print(" ".join(packages))


if __name__ == "__main__":
    main()
