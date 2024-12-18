# /// script
# requires-python = ">=3.9"
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
import re


def update_version(pyproject_path: Path, new_version: str):
    with open(pyproject_path, "r") as file:
        pyproject_data = tomlkit.parse(file.read())

    pyproject_data["project"]["version"] = new_version  # type: ignore

    with open(pyproject_path, "w") as file:
        file.write(tomlkit.dumps(pyproject_data))


def generate_next_version(sdk: Path) -> str:
    with open(sdk, "r") as file:
        pyproject_data = tomlkit.parse(file.read())
    current = str(pyproject_data["project"]["version"])  # type: ignore
    return next_version(current)


def next_version(version: str) -> str:
    """
    Generate the next version from the given version string.

    Args:
        version (str): The version string (e.g., '1.2.3' or '1.2.3rc1').

    Returns:
        str: The next version string.
    """
    # Regular expression for parsing versions
    match = re.match(r"^(\d+)\.(\d+)\.(\d+)(?:rc(\d+))?$", version)
    if not match:
        raise ValueError(f"Invalid version format: {version}")

    major, minor, patch, rc = match.groups()
    major, minor, patch = int(major), int(minor), int(patch)

    if rc is not None:
        # If it is a release candidate, increment the rc number
        rc = int(rc) + 1
        return f"{major}.{minor}.{patch}rc{rc}"
    else:
        # Otherwise, increment the patch version
        patch += 1
        return f"{major}.{minor}.{patch}"


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
        new_version = generate_next_version(sdk)
    print(new_version)

    for pyproject in [sdk] + adapters:
        update_version(pyproject, new_version)


if __name__ == "__main__":
    main()
