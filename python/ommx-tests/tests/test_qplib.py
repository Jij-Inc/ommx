from pathlib import Path
import os
import subprocess
import sys

import pytest

from ommx import Instance, State, qplib


FIXTURES = Path(__file__).resolve().parents[3] / "rust/ommx/tests/fixtures"


def test_dataset_uses_corrected_distribution_with_legacy_cache(tmp_path: Path) -> None:
    # Isolate the process-global registry root from other tests and the user's cache.
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            """
from pathlib import Path
import sys
from ommx import Instance, State
from ommx.artifact import Artifact, ArtifactDraft
from ommx.dataset import qplib

fixtures = Path(sys.argv[1])
legacy_ref = "ghcr.io/jij-inc/ommx/qplib:0018"
corrected_ref = "ghcr.io/jij-inc/ommx/v2.8/qplib:0018"
corrected = Instance.load_qplib(str(fixtures / "QPLIB_0018.qplib"))
for ref, model in [(legacy_ref, Instance.empty()), (corrected_ref, corrected)]:
    draft = ArtifactDraft.new(ref)
    draft.add_layer(
        "application/org.ommx.v1.instance",
        model.to_v1_bytes(),
        {
            "org.ommx.v1.instance.title": "QPLIB_0018",
            "org.ommx.qplib.parser_version": "2.8.0",
        },
    )
    draft.commit()

instance = qplib("0018")
assert instance.title == "QPLIB_0018"
assert instance.annotations["org.ommx.qplib.parser_version"] == "2.8.0"
assert len(instance.decision_variables) == 50
state = State.load_qplib_solution(str(fixtures / "QPLIB_0018.sol"), num_variables=50)
solution = instance.evaluate(state, atol=1e-8)
assert abs(solution.objective - (-6.38601498159835)) < 1e-10
assert solution.feasible
assert len(Artifact.load(legacy_ref).instance.decision_variables) == 0
""",
            str(FIXTURES),
        ],
        env={**os.environ, "OMMX_LOCAL_REGISTRY_ROOT": str(tmp_path)},
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr


@pytest.mark.parametrize(
    ("tag", "expected"),
    [("0018", -6.386014981598350), ("0681", 45.244448166487501)],
)
def test_published_qplib_solution(tag: str, expected: float) -> None:
    instance = Instance.load_qplib(str(FIXTURES / f"QPLIB_{tag}.qplib"))
    state = State.load_qplib_solution(
        str(FIXTURES / f"QPLIB_{tag}.sol"),
        num_variables=len(instance.decision_variables),
    )
    assert isinstance(state, State)
    assert set(state.entries) == {
        variable.id for variable in instance.decision_variables
    }
    solution = instance.evaluate(state, atol=1e-8)
    assert solution.objective == pytest.approx(expected, abs=1e-10, rel=0)
    assert solution.feasible


def test_solution_loader_fills_omitted_variables(tmp_path: Path) -> None:
    path = tmp_path / "solution.sol"
    path.write_text("objvar -12\nx2 0.5\nb4 1\ni5 -2\n")
    state = qplib.load_solution(str(path), num_variables=5)
    assert state.entries == {0: 0.5, 1: 0.0, 2: 1.0, 3: -2.0, 4: 0.0}


@pytest.mark.parametrize(
    ("text", "message"),
    [
        ("# comment\nx2\n", "line 2: expected a variable name and a numeric value"),
        ("x2 1\nb2 1\n", "line 2: duplicate variable ID 0"),
        ("x5 1\n", "line 1: .*outside 0..3"),
        ("x2 NaN\n", "line 1: solution values must be finite"),
        ("custom_name 1\n", "line 1: invalid variable name"),
    ],
)
def test_solution_errors_are_value_errors(
    tmp_path: Path, text: str, message: str
) -> None:
    path = tmp_path / "invalid.sol"
    path.write_text(text)
    with pytest.raises(ValueError, match=message):
        State.load_qplib_solution(str(path), num_variables=3)


def test_missing_solution_file_is_runtime_error(tmp_path: Path) -> None:
    with pytest.raises(RuntimeError, match="Failed to read QPLIB solution"):
        State.load_qplib_solution(str(tmp_path / "missing.sol"), num_variables=3)
