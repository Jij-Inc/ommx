import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import tomlkit
from ruamel.yaml import YAML


REPO = Path(__file__).resolve().parents[2]
WHEELS = {
    "abi3": "ommx-3.0.0b9-cp311-abi3-manylinux_2_28_x86_64.whl",
    "cp314t": "ommx-3.0.0b9-cp314-cp314t-manylinux_2_28_x86_64.whl",
    "abi3t": "ommx-3.0.0b9-cp315-abi3t.abi3-manylinux_2_28_x86_64.whl",
}


class SetupPythonTestProjectTests(unittest.TestCase):
    def test_ci_workspace_matches_interpreter_and_test_scope(self):
        cases = [
            ("3.11", False, "abi3"),
            ("3.12", False, "abi3"),
            ("3.14t", True, "cp314t"),
            ("3.15", True, "abi3t"),
            ("3.15t", True, "abi3t"),
        ]
        for version, sdk_only, abi in cases:
            with self.subTest(version=version), tempfile.TemporaryDirectory() as tmp:
                project = Path(tmp)
                shutil.copy(REPO / "pyproject.toml", project)
                (project / "python").mkdir()
                shutil.copy(REPO / "python/Taskfile.yml", project / "python")
                for metadata in (REPO / "python").glob("ommx*/pyproject.toml"):
                    package = project / "python" / metadata.parent.name
                    package.mkdir()
                    shutil.copy(metadata, package)
                (project / "wheels").mkdir()
                for name in WHEELS.values():
                    (project / "wheels" / name).touch()

                command = [
                    sys.executable,
                    str(REPO / "ci-scripts/setup-python-test-project.py"),
                    version,
                ]
                if sdk_only:
                    command.append("--sdk-only")
                subprocess.run(command, cwd=project, check=True, capture_output=True)
                config = tomlkit.parse((project / "pyproject.toml").read_text())
                uv = config["tool"]["uv"]
                groups = config["dependency-groups"]
                self.assertEqual(
                    uv["environments"], [f"python_version == '{version.rstrip('t')}'"]
                )
                self.assertEqual(uv["sources"]["ommx"]["path"], f"wheels/{WHEELS[abi]}")
                self.assertTrue(
                    any(dep.startswith("sphinx>=") for dep in groups["docs"])
                )
                self.assertEqual("tutorials" in groups, not sdk_only)
                self.assertEqual(
                    {"include-group": "tutorials"} in groups["dev"], not sdk_only
                )
                self.assertIn("python/ommx-tests", uv["workspace"]["members"])
                for adapter in ["highs", "pyscipopt"]:
                    self.assertIn(
                        f"python/ommx-{adapter}-adapter", uv["workspace"]["members"]
                    )
                tasks = YAML().load((project / "python/Taskfile.yml").read_text())
                if sdk_only:
                    self.assertEqual(
                        tasks["tasks"]["test-ci"]["cmds"], [{"task": "ommx:pytest"}]
                    )
                else:
                    self.assertEqual(len(tasks["tasks"]["test-ci"]["cmds"]), 5)
                # Interpreter restrictions belong to this CI workspace, never
                # to the packages' published requires-python metadata.
                for metadata in (project / "python").glob("ommx*/pyproject.toml"):
                    self.assertEqual(
                        metadata.read_text(),
                        (
                            REPO / "python" / metadata.parent.name / "pyproject.toml"
                        ).read_text(),
                    )
