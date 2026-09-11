import os
from pathlib import Path
import subprocess
import sys


def test_qplib_uses_versioned_distribution_with_legacy_cache(tmp_path):
    fixture = (
        Path(__file__).resolve().parents[3]
        / "rust/ommx/tests/fixtures/QPLIB_0018.qplib"
    )
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            """
import sys
from ommx.artifact import ArtifactBuilder
from ommx.dataset import qplib
from ommx.v1 import Instance

old = ArtifactBuilder.for_github("Jij-Inc", "ommx", "qplib", "0018")
old.add_instance(Instance.empty())
old.build()
new = ArtifactBuilder.for_github("Jij-Inc", "ommx", "v2.8/qplib", "0018")
new.add_instance(Instance.load_qplib(sys.argv[1]))
new.build()

instance = qplib("0018")
assert len(instance.decision_variables) == 50
state = {i: 0.0 for i in range(50)}
state.update({13: 0.209636569541294, 16: 0.275230558068530,
              38: 0.226997921553671, 40: 0.288134950836505})
solution = instance.evaluate(state, atol=1e-8)
assert abs(solution.objective - (-6.38601498159835)) < 1e-10
assert solution.feasible
""",
            str(fixture),
        ],
        env={**os.environ, "OMMX_LOCAL_REGISTRY_ROOT": str(tmp_path)},
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr


def test_miplib2017_uses_versioned_distribution_with_legacy_cache(tmp_path):
    fixture = (
        Path(__file__).resolve().parents[3]
        / "rust/ommx/tests/data/mps/neos-2626858-aoos.mps.gz"
    )
    # The registry root is process-global, so isolate this test from other tests
    # and from the user's real Artifact cache.
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            """
import sys
from ommx.artifact import ArtifactBuilder
from ommx.dataset import miplib2017
from ommx.v1 import Instance, DecisionVariable

name = "neos-2626858-aoos"
old = ArtifactBuilder.for_github("Jij-Inc", "ommx", "miplib2017", name)
old.add_instance(Instance.empty())
old.build()

new = ArtifactBuilder.for_github("Jij-Inc", "ommx", "v2.7/miplib2017", name)
new.add_instance(Instance.load_mps(sys.argv[1]))
new.build()

instance = miplib2017(name)
assert len(instance.decision_variables) == 524
assert len(instance.constraints) == 342
assert sum(v.kind == DecisionVariable.BINARY for v in instance.decision_variables) == 209
assert sum(v.kind == DecisionVariable.INTEGER for v in instance.decision_variables) == 315
targets = {f"C{i:04d}" for i in range(493, 509)} | {"C0524"}
affected = [v for v in instance.decision_variables if v.name in targets]
assert len(affected) == 17
assert all(v.kind == DecisionVariable.BINARY and v.bound.lower == 0 and v.bound.upper == 1 for v in affected)
""",
            str(fixture),
        ],
        env={**os.environ, "OMMX_LOCAL_REGISTRY_ROOT": str(tmp_path)},
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr
