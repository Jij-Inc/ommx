"""Compile the same consumer against current and extended generated kind APIs."""

import json
from pathlib import Path
import re
import subprocess


def test_consumer_survives_kind_and_family_additions(tmp_path: Path):
    root = Path(__file__).resolve().parents[3]
    stub = (root / "python/ommx/ommx/_ommx_rust/__init__.pyi").read_text()
    classes = []
    names = ["Kind", "ConstraintKind", "ProvenanceKind"]
    for name in names:
        match = re.search(
            rf"@typing.final\nclass {name}(?:\([^\n]*\))?:\n.*?(?=\n@typing.final\nclass )",
            stub,
            re.DOTALL,
        )
        assert match is not None
        classes.append(match.group())
    # This consumer must remain identical as the SDK adds known constants.
    consumer = (Path(__file__).with_name("test_open_kinds.py")).read_text()
    consumer = consumer.replace("from ommx import", "from kind_api import")
    for extended in [False, True]:
        api = (
            "import builtins\nimport typing\nimport enum\n"
            "from ommx._typing import _C_co\n"
            "from ommx import ConstraintIDBase, ConstraintID, IndicatorConstraintID, OneHotConstraintID, Sos1ConstraintID\n"
            + "\n".join(classes)
        )
        if extended:
            for name in names:
                annotation = (
                    f"{name}[ConstraintIDBase]" if name == "ConstraintKind" else name
                )
                api = re.sub(
                    rf"(class {name}(?:\([^\n]*\))?:\n)",
                    rf"\1    Future: {annotation}\n",
                    api,
                )
        (tmp_path / "kind_api.pyi").write_text(api)
        path = tmp_path / "consumer.py"
        path.write_text(consumer)
        config = tmp_path / "pyrightconfig.json"
        config.write_text(json.dumps({"pythonVersion": "3.10"}))
        result = subprocess.run(
            ["uv", "run", "--no-sync", "pyright", "--project", str(config), str(path)],
            cwd=root,
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0, result.stdout + result.stderr
