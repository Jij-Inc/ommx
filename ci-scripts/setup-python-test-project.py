"""Configure a disposable uv workspace to test the wheel downloaded by CI."""

import argparse
import platform
import sys
from pathlib import Path

import tomlkit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", help="Target Python minor, e.g. 3.15 or 3.15t")
    parser.add_argument("wheel", type=Path)
    parser.add_argument("--sdk-only", action="store_true")
    args = parser.parse_args()
    if not args.wheel.is_file():
        parser.error(f"Missing SDK wheel: {args.wheel}")

    project = Path("pyproject.toml")
    config = tomlkit.parse(project.read_text())
    uv = config["tool"]["uv"]
    uv["sources"]["ommx"] = {"path": str(args.wheel)}
    uv["environments"] = [
        f"python_version == '{args.version.removesuffix('t')}' and "
        f"sys_platform == '{sys.platform}' and platform_machine == '{platform.machine()}'"
    ]
    uv["workspace"]["exclude"] = ["python/ommx"]
    if args.sdk_only:
        # The SDK doctests exercise both solver adapters.
        uv["workspace"]["members"] = [
            "python/ommx-tests",
            "python/ommx-highs-adapter",
            "python/ommx-pyscipopt-adapter",
        ]
        uv["default-groups"] = ["benchmark", "docs", "build", "typecheck"]
    project.write_text(tomlkit.dumps(config))


if __name__ == "__main__":
    main()
