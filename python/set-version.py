# /// script
# requires-python = ">=3.8"
# dependencies = [
#   "tomlkit"
# ]
# ///

# Bump up versions of OMMX Python SDK and adapters
#
# Usage
# ------
#
# Set the version of the OMMX Python SDK and adapters to the specified version.
#
# ```shell
# uv run python/set-version.py 0.1.0
# ```
#
# Bump up patch version of the OMMX Python SDK and adapters
#
# ```shell
# uv run python/set-version.py
# ```
#

from pathlib import Path
import tomlkit
import argparse


def update_version(pyproject_path: Path, new_version: str):
    with open(pyproject_path, "r") as file:
        pyproject_data = tomlkit.parse(file.read())

    pyproject_data["project"]["version"] = new_version  # type: ignore

    with open(pyproject_path, "w") as file:
        file.write(tomlkit.dumps(pyproject_data))


def generrate_next_version(sdk: Path) -> str:
    with open(sdk, "r") as file:
        pyproject_data = tomlkit.parse(file.read())
    current = str(pyproject_data["project"]["version"])  # type: ignore
    major, minor, patch = current.split(".")
    return f"{major}.{minor}.{int(patch) + 1}"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("version", help="New version to set", default=None, nargs="?")

    args = parser.parse_args()

    here = Path(__file__).parent
    sdk = here / "ommx" / "pyproject.toml"
    adapters = [
        here / name / "pyproject.toml"
        for name in [
            "ommx-pyscipopt-adapter",
            "ommx-python-mip-adapter",
            # Add new adapter here
        ]
    ]

    if args.version:
        new_version = args.version
    else:
        new_version = generrate_next_version(sdk)

    for pyproject in [sdk] + adapters:
        update_version(pyproject, new_version)


if __name__ == "__main__":
    main()
