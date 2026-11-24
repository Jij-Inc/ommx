import subprocess
import sys

import pytest

from ommx import DecisionVariableRole, Equality, Optimality, Relaxation, Sense


@pytest.mark.parametrize(
    ("member", "expected_repr", "expected_str"),
    [
        (Sense.Minimize, "Sense.Minimize", "1"),
        (Sense.Maximize, "Sense.Maximize", "2"),
        (Equality.EqualToZero, "Equality.EqualToZero", "=0"),
        (Equality.LessThanOrEqualToZero, "Equality.LessThanOrEqualToZero", "<=0"),
        (Optimality.Unspecified, "Optimality.Unspecified", "0"),
        (Optimality.Optimal, "Optimality.Optimal", "1"),
        (Optimality.NotOptimal, "Optimality.NotOptimal", "2"),
        (Relaxation.Unspecified, "Relaxation.Unspecified", "0"),
        (Relaxation.LpRelaxed, "Relaxation.LpRelaxed", "1"),
        (DecisionVariableRole.Active, "DecisionVariableRole.Active", "active"),
        (DecisionVariableRole.Fixed, "DecisionVariableRole.Fixed", "fixed"),
        (DecisionVariableRole.Dependent, "DecisionVariableRole.Dependent", "dependent"),
        (
            DecisionVariableRole.Irrelevant,
            "DecisionVariableRole.Irrelevant",
            "irrelevant",
        ),
    ],
)
def test_enum_display(member: object, expected_repr: str, expected_str: str) -> None:
    assert repr(member) == expected_repr
    assert str(member) == expected_str


def test_import_has_no_duplicate_repr_slots() -> None:
    # A fresh interpreter is needed: pytest has already imported the extension.
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            """
import warnings

with warnings.catch_warnings(record=True) as captured:
    warnings.simplefilter("always", DeprecationWarning)
    import ommx

duplicates = [str(w.message) for w in captured if "multiple Py_tp_repr" in str(w.message)]
assert not duplicates, duplicates
""",
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
