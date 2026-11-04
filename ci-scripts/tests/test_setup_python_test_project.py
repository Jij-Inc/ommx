import platform
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

from packaging.markers import Marker


REPO = Path(__file__).resolve().parents[2]


class TestWheelWorkspace(unittest.TestCase):
    def test_workspace_scope(self):
        for version, flags in [("3.12", []), ("3.15t", ["--sdk-only"])]:
            with self.subTest(version=version), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                project = root / "pyproject.toml"
                shutil.copy(REPO / "pyproject.toml", project)
                wheel = (
                    Path("ommx-1-cp315-abi3.abi3t-linux_x86_64.whl")
                    if flags
                    else Path("ommx-1-cp311-abi3-linux_x86_64.whl")
                )
                (root / wheel).touch()
                subprocess.run(
                    [
                        sys.executable,
                        REPO / "ci-scripts/setup-python-test-project.py",
                        version,
                        wheel,
                        *flags,
                    ],
                    cwd=root,
                    check=True,
                )
                uv = tomllib.loads(project.read_text())["tool"]["uv"]
                self.assertEqual(uv["sources"]["ommx"], {"path": str(wheel)})
                marker = Marker(uv["environments"][0])
                target = {
                    "python_version": version.rstrip("t"),
                    "sys_platform": sys.platform,
                    "platform_machine": platform.machine(),
                }
                self.assertTrue(marker.evaluate(target))
                for key, value in [
                    ("python_version", "3.10"),
                    ("sys_platform", "emscripten"),
                    ("platform_machine", "wasm32"),
                ]:
                    self.assertFalse(marker.evaluate(target | {key: value}))
                self.assertIn("python/ommx", uv["workspace"]["exclude"])
                if flags:
                    self.assertCountEqual(
                        uv["workspace"]["members"],
                        [
                            "python/ommx-tests",
                            "python/ommx-highs-adapter",
                            "python/ommx-pyscipopt-adapter",
                        ],
                    )
                    self.assertCountEqual(
                        uv["default-groups"],
                        ["benchmark", "docs", "build", "typecheck"],
                    )
                else:
                    self.assertEqual(uv["workspace"]["members"], ["python/ommx*"])
                    self.assertNotIn("default-groups", uv)
