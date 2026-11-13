"""Static contracts for open diagnostic maps and concrete family views."""

from collections.abc import Mapping
from typing import TYPE_CHECKING
from typing_extensions import assert_type
from ommx import (
    ConstraintID,
    ConstraintIDBase,
    ConstraintKind,
    IndicatorConstraintID,
    OneHotConstraintID,
    Sos1ConstraintID,
    Solution,
    SampleSet,
    ViolationDiagnostics,
    ViolationDiagnosticsBase,
    ViolationDiagnosticsFamilyView,
    RegularViolationDiagnostics,
    IndicatorViolationDiagnostics,
    OneHotViolationDiagnostics,
    Sos1ViolationDiagnostics,
    OneHotValueErrors,
)
from test_violation_diagnostics import model


class Missing:
    pass


def check_types(
    solution: Solution,
    samples: SampleSet,
    key: ConstraintIDBase,
    default: ViolationDiagnosticsBase,
) -> None:
    rows = solution.violation_diagnostics()
    assert_type(rows, ViolationDiagnostics)
    assert_type(samples.violation_diagnostics(7), ViolationDiagnostics)
    mapping: Mapping[ConstraintIDBase, ViolationDiagnosticsBase] = rows
    assert_type(rows[key], ViolationDiagnosticsBase)
    assert_type(rows.get(key), ViolationDiagnosticsBase | None)
    assert_type(rows.get(key, Missing()), ViolationDiagnosticsBase | Missing)
    assert_type(rows.get(key, default), ViolationDiagnosticsBase)
    assert_type(mapping.get(key, Missing()), ViolationDiagnosticsBase | Missing)
    assert_type(rows[ConstraintID(0)], RegularViolationDiagnostics)
    assert_type(rows[IndicatorConstraintID(10)], IndicatorViolationDiagnostics)
    assert_type(rows[OneHotConstraintID(20)], OneHotViolationDiagnostics)
    assert_type(rows[Sos1ConstraintID(30)], Sos1ViolationDiagnostics)
    assert_type(
        rows.get(ConstraintID(0), Missing()), RegularViolationDiagnostics | Missing
    )
    assert_type(
        rows.get(IndicatorConstraintID(10), Missing()),
        IndicatorViolationDiagnostics | Missing,
    )
    assert_type(
        rows.get(OneHotConstraintID(20), Missing()),
        OneHotViolationDiagnostics | Missing,
    )
    assert_type(
        rows.get(Sos1ConstraintID(30), Missing()), Sos1ViolationDiagnostics | Missing
    )
    assert_type(rows.get(Sos1ConstraintID(30), None), Sos1ViolationDiagnostics | None)
    assert_type(rows.get(Sos1ConstraintID(30)), Sos1ViolationDiagnostics | None)
    regular = rows.of_kind(ConstraintKind.Regular)
    indicator = rows.of_kind(ConstraintKind.Indicator)
    one_hot = rows.of_kind(ConstraintKind.OneHot)
    sos1 = rows.of_kind(ConstraintKind.Sos1)
    assert_type(
        regular,
        ViolationDiagnosticsFamilyView[ConstraintID, RegularViolationDiagnostics],
    )
    assert_type(
        indicator,
        ViolationDiagnosticsFamilyView[
            IndicatorConstraintID, IndicatorViolationDiagnostics
        ],
    )
    assert_type(
        one_hot,
        ViolationDiagnosticsFamilyView[OneHotConstraintID, OneHotViolationDiagnostics],
    )
    assert_type(
        sos1, ViolationDiagnosticsFamilyView[Sos1ConstraintID, Sos1ViolationDiagnostics]
    )
    assert_type(regular[0], RegularViolationDiagnostics)
    assert_type(indicator[10], IndicatorViolationDiagnostics)
    assert_type(one_hot[OneHotConstraintID(20)], OneHotViolationDiagnostics)
    assert_type(sos1[30], Sos1ViolationDiagnostics)
    assert_type(sos1.get(999, Missing()), Sos1ViolationDiagnostics | Missing)
    assert_type(next(iter(sos1)), Sos1ConstraintID)
    assert_type(next(iter(sos1.values())), Sos1ViolationDiagnostics)
    assert_type(
        next(iter(sos1.items())), tuple[Sos1ConstraintID, Sos1ViolationDiagnostics]
    )
    detail = one_hot[20]
    if isinstance(detail, OneHotValueErrors):
        assert_type(detail.errors, dict[int, float])
    assert isinstance(rows[ConstraintID(0)], ViolationDiagnosticsBase)


def test_static_contracts_also_run():
    instance = model()
    state = {0: 1, 1: 1, 2: 1}
    solution = instance.evaluate(state)
    check_types(
        solution,
        instance.evaluate_samples({7: state}),
        ConstraintID(0),
        solution.violation_diagnostics()[ConstraintID(0)],
    )


if TYPE_CHECKING:
    # Runtime and stub both provide a real generic class alias.
    Family = ViolationDiagnosticsFamilyView[Sos1ConstraintID, Sos1ViolationDiagnostics]


def check_future_family(
    rows: ViolationDiagnostics, kind: ConstraintKind[ConstraintIDBase]
) -> None:
    assert_type(
        rows.of_kind(kind),
        ViolationDiagnosticsFamilyView[ConstraintIDBase, ViolationDiagnosticsBase],
    )
    detail = rows[OneHotConstraintID(20)]
    if isinstance(detail, OneHotViolationDiagnostics.ValueErrors):
        assert_type(detail, OneHotValueErrors)
        assert_type(detail.values, dict[int, float])


def test_static_rejection_of_untyped_and_wrong_family_keys(tmp_path):
    import json
    from pathlib import Path
    import subprocess

    source = tmp_path / "consumer.py"
    source.write_text(
        "from ommx import ViolationDiagnostics, ConstraintKind, Sos1ConstraintID\n"
        "def inspect(rows: ViolationDiagnostics):\n"
        "    rows[0]\n"
        "    rows.get(0)\n"
        "    family = rows.of_kind(ConstraintKind.OneHot)\n"
        "    family[Sos1ConstraintID(7)]\n"
        "    family.get(Sos1ConstraintID(7))\n"
    )
    root = Path(__file__).resolve().parents[3]
    config = tmp_path / "pyrightconfig.json"
    config.write_text(
        json.dumps({"pythonVersion": "3.10", "venvPath": str(root), "venv": ".venv"})
    )
    result = subprocess.run(
        [
            "uv",
            "run",
            "--no-sync",
            "pyright",
            "--outputjson",
            "--project",
            str(config),
            str(source),
        ],
        cwd=root,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1, result.stdout + result.stderr
    errors = [
        d
        for d in json.loads(result.stdout)["generalDiagnostics"]
        if d["severity"] == "error"
    ]
    assert {d["range"]["start"]["line"] for d in errors} == {2, 3, 5, 6}, errors
