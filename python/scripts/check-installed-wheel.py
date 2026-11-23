"""Ensure a wheel-consuming check has not replaced the SDK with a source build."""

import json
import sys
from importlib.metadata import distribution
from pathlib import Path


def main() -> None:
    (wheel,) = Path(sys.argv[1]).glob("ommx-*.whl")
    metadata = distribution("ommx").read_text("direct_url.json")
    assert metadata is not None, "Installed SDK has no direct wheel origin"
    source = json.loads(metadata)
    assert source["url"] == wheel.resolve().as_uri(), source


if __name__ == "__main__":
    main()
