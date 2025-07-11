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
    # Update the version of the OMMX dependency
    for dep in pyproject_data["project"]["dependencies"]:  # type: ignore
        if re.match(r"ommx\s*>=\s*\d+\.\d+\.\d+,\s*<\s*\d+\.\d+\.\d+", dep):
            new_dep = re.sub(r"\d+\.\d+\.\d+", new_version, dep, count=1)
            pyproject_data["project"]["dependencies"].remove(dep)  # type: ignore
            pyproject_data["project"]["dependencies"].insert(0, new_dep)  # type: ignore

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
        version (str): The version string (e.g., '1.2.3', '1.2.3rc1', '1.2.3a1', '1.2.3b2').

    Returns:
        str: The next version string.
    """
    # Regular expression for parsing versions with alpha, beta, and rc support
    match = re.match(r"^(\d+)\.(\d+)\.(\d+)(?:(a|b|rc)(\d+))?$", version)
    if not match:
        raise ValueError(f"Invalid version format: {version}")

    major, minor, patch, pre_type, pre_num = match.groups()
    major, minor, patch = int(major), int(minor), int(patch)

    if pre_type is not None:
        # If it has a pre-release identifier, increment the pre-release number
        pre_num = int(pre_num) + 1
        return f"{major}.{minor}.{patch}{pre_type}{pre_num}"
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
            "ommx-tests",
            "ommx-pyscipopt-adapter",
            "ommx-python-mip-adapter",
            "ommx-openjij-adapter",
            "ommx-highs-adapter",
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
