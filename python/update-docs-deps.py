# /// script
# requires-python = ">=3.9"
# dependencies = [
#   "tomlkit",
#   "requests"
# ]
# ///

# Update docs/pyproject.toml to use released OMMX packages
#
# Usage
# ------
#
# Update docs dependencies to use the specified OMMX version:
#
# ```shell
# uv run python/update-docs-deps.py 2.0.10
# ```
#
# Or fetch the latest version from PyPI:
#
# ```shell
# uv run python/update-docs-deps.py
# ```

from pathlib import Path
import tomlkit
import argparse
import re
import requests


def update_docs_dependencies(docs_pyproject: Path, version: str):
    """Update docs/pyproject.toml with the specified OMMX version."""
    with open(docs_pyproject, "r") as file:
        pyproject_data = tomlkit.parse(file.read())

    # Update project version to match OMMX version
    project = pyproject_data["project"]
    assert isinstance(project, dict)
    project["version"] = version

    # Update all ommx* dependencies to the specified version
    dependencies = project["dependencies"]
    assert isinstance(dependencies, list)
    updated_dependencies = []

    for dep in dependencies:
        assert isinstance(dep, str)
        # Match ommx packages (ommx, ommx-*-adapter, etc.)
        if re.match(r"ommx[a-z0-9\-]*>=", dep):
            # Extract package name
            pkg_name = dep.split(">=")[0].strip()
            # Replace with new version constraint
            updated_dep = f"{pkg_name}>={version}"
            updated_dependencies.append(updated_dep)
        else:
            updated_dependencies.append(dep)

    project["dependencies"] = updated_dependencies

    with open(docs_pyproject, "w") as file:
        file.write(tomlkit.dumps(pyproject_data))


def get_latest_ommx_version() -> str:
    """Fetch the latest OMMX version from PyPI."""
    response = requests.get("https://pypi.org/pypi/ommx/json", timeout=10)
    response.raise_for_status()
    data = response.json()
    version = data["info"]["version"]
    assert isinstance(version, str)
    return version


def main():
    parser = argparse.ArgumentParser(
        description="Update docs/pyproject.toml to use released OMMX packages"
    )
    parser.add_argument(
        "version",
        nargs="?",
        help="OMMX version to set (e.g., 2.0.10). If not provided, fetches the latest from PyPI.",
    )

    args = parser.parse_args()

    here = Path(__file__).parent
    docs_pyproject = here.parent / "docs" / "pyproject.toml"

    if not docs_pyproject.exists():
        raise FileNotFoundError(f"docs/pyproject.toml not found at {docs_pyproject}")

    if args.version:
        version = args.version
    else:
        print("Fetching latest OMMX version from PyPI...")
        version = get_latest_ommx_version()
        print(f"Found latest version: {version}")

    print(f"Updating docs/pyproject.toml to OMMX version {version}")
    update_docs_dependencies(docs_pyproject, version)
    print("✓ Updated successfully")


if __name__ == "__main__":
    main()
