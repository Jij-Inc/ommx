"""Snapshot tests for the unified `constraints_df(kind=, include=, min_role=)`
method on Instance / ParametricInstance / Solution / SampleSet.

Wave 2 of the v3-alpha pandas surface collapses 26 per-kind methods into
4 unified `constraints_df` accessors. The `.ambr` snapshots are the
authoritative description of the column / index schema for each host ×
kind × include / role combination — update via
`pytest --snapshot-update` after a deliberate API change.
"""

from __future__ import annotations
from ommx import ConstraintRole, ConstraintKind
from ommx import ConstraintID, OneHotConstraintID

import pandas as pd
import pytest
from ommx import (
    Sense,
    Constraint,
    DecisionVariable,
    Equality,
    Function,
    IndicatorConstraint,
    Instance,
    OneHotConstraint,
    Parameter,
    ParametricInstance,
    Sos1Constraint,
)


def _df_snap(df: pd.DataFrame) -> str:
    """Deterministic, snapshot-friendly rendering of a DataFrame."""
    return df.to_string(na_rep="<NA>")


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _instance_all_kinds() -> Instance:
    """Instance with one constraint of each kind, all carrying context."""
    x = [
        DecisionVariable.binary(0, name="x0"),
        DecisionVariable.binary(1, name="x1"),
        DecisionVariable.binary(2, name="x2"),
    ]
    c = (
        (x[0] + x[1] + x[2] == 1)
        .set_name("balance")
        .set_subscripts([0, 1])
        .set_description("demand row")
        .set_parameters({"region": "us-east"})
    )
    assert isinstance(c, Constraint)
    return Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={10: c},
        active_indicator_constraints={
            20: IndicatorConstraint(
                indicator_variable=x[0],
                function=x[1] + x[2] - 1,
                equality=Equality.EqualToZero,
            )
        },
        active_one_hot_constraints={30: OneHotConstraint(variables=x)},
        active_sos1_constraints={40: Sos1Constraint(variables=x)},
        sense=Sense.Maximize,
    )


def _instance_with_inactive() -> Instance:
    """Instance whose regular constraint at id=10 has been relaxed."""
    inst = _instance_all_kinds()
    inst.deactivate_constraint(10, "manual_relax", phase="warm-up")
    return inst


def _parametric_instance() -> ParametricInstance:
    """ParametricInstance with one regular constraint that uses a parameter."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    p = Parameter(99, name="p")
    c = x[0] + x[1] + p * x[2] == 1
    assert isinstance(c, Constraint)
    return ParametricInstance.from_components(
        sense=Sense.Maximize,
        objective=sum(x),
        decision_variables=x,
        active_constraints={10: c},
        parameters=[p],
    )


def _solution_basic():
    """Solution from `_instance_all_kinds()` evaluated at a feasible point."""
    inst = _instance_all_kinds()
    return inst.evaluate({0: 0.0, 1: 1.0, 2: 0.0})


def _solution_with_inactive():
    """Solution where the regular constraint at id=10 was relaxed before evaluation."""
    inst = _instance_with_inactive()
    return inst.evaluate({0: 0.0, 1: 1.0, 2: 0.0})


def _sample_set_basic():
    """SampleSet with two samples spanning feasible / infeasible."""
    inst = _instance_all_kinds()
    return inst.evaluate_samples(
        {
            0: {0: 0.0, 1: 1.0, 2: 0.0},
            1: {0: 1.0, 1: 1.0, 2: 0.0},
        }
    )


# ---------------------------------------------------------------------------
# Instance.constraints_df — kind dispatch
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "kind",
    [
        ConstraintKind.Regular,
        ConstraintKind.Indicator,
        ConstraintKind.OneHot,
        ConstraintKind.Sos1,
    ],
    ids=str,
)
def test_instance_constraints_df_kind(snapshot, kind):
    """Default `include=` (label + parameters); index is `{kind}_constraint_id`."""
    assert _df_snap(_instance_all_kinds().constraints_df(kind=kind)) == snapshot


def test_instance_constraints_df_unknown_kind():
    """The selector must be a ConstraintKind value, even without a type checker."""
    with pytest.raises(TypeError, match="ConstraintKind"):
        _instance_all_kinds().constraints_df(kind="bogus")  # type: ignore[arg-type]


def test_instance_constraints_df_reports_composed_function_type():
    variable = DecisionVariable.continuous(0)
    function = Function(variable)
    constraint = abs(function) <= 1
    assert isinstance(constraint, Constraint)
    instance = Instance.from_components(
        decision_variables=[variable],
        objective=variable,
        active_constraints={10: constraint},
        sense=Sense.Minimize,
    )

    assert (
        instance.constraints_df(kind=ConstraintKind.Regular).loc[
            ConstraintID(10), "type"
        ]
        == "Expression"
    )


def test_instance_constraints_df_unknown_include_flag():
    """Unknown `include=` flag raises ValueError."""
    with pytest.raises(ValueError, match="unknown include flag"):
        _instance_all_kinds().constraints_df(
            kind=ConstraintKind.Regular, include=["bogus"]
        )


# ---------------------------------------------------------------------------
# Instance.constraints_df — include= matrix
# ---------------------------------------------------------------------------


def test_instance_constraints_df_include_empty(snapshot):
    """`include=[]` strips label + parameters columns; only core columns remain."""
    assert (
        _df_snap(
            _instance_all_kinds().constraints_df(
                kind=ConstraintKind.Regular, include=[]
            )
        )
        == snapshot
    )


def test_instance_constraints_df_include_label_only(snapshot):
    """`include=("label",)` keeps name/subscripts/description, drops parameters."""
    assert (
        _df_snap(
            _instance_all_kinds().constraints_df(
                kind=ConstraintKind.Regular, include=["label"]
            )
        )
        == snapshot
    )


def test_instance_constraints_df_include_parameters_only(snapshot):
    """`include=("parameters",)` keeps parameters.{key}, drops name/subscripts/description."""
    assert (
        _df_snap(
            _instance_all_kinds().constraints_df(
                kind=ConstraintKind.Regular, include=["parameters"]
            )
        )
        == snapshot
    )


# ---------------------------------------------------------------------------
# Instance.constraints_df — min_role=
# ---------------------------------------------------------------------------


def test_instance_constraints_df_active_only_excludes_inactive(snapshot):
    """The ACTIVE threshold excludes the deactivated constraint at id=10."""
    assert (
        _df_snap(
            _instance_with_inactive().constraints_df(
                kind=ConstraintKind.Regular,
                min_role=ConstraintRole.ACTIVE,
            )
        )
        == snapshot
    )


def test_instance_constraints_df_default_includes_inactive(snapshot):
    """The default displays every retained state, including inactive rows."""
    assert (
        _df_snap(
            _instance_with_inactive().constraints_df(
                kind=ConstraintKind.Regular,
                include=["label", "parameters", "inactive_reason"],
            )
        )
        == snapshot
    )


def test_instance_constraints_df_inactive_reason_without_label(snapshot):
    """Inactive reasons can be requested without the modeling-label columns."""
    assert (
        _df_snap(
            _instance_with_inactive().constraints_df(
                kind=ConstraintKind.Regular, include=["inactive_reason"]
            )
        )
        == snapshot
    )


def test_instance_constraints_df_all_states_id_sorted(snapshot):
    """An all-state query returns a globally id-sorted union of active and
    inactive rows. With active ids {5, 30} and inactive id 20 in the
    middle, the output is [5, 20, 30] — not [5, 30, 20] (active first
    then inactive). Regression guard for the merge-sort path; the
    naive `chain(active, inactive)` would order rows by section
    rather than by id."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    inst = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={
            5: x[0] + x[1] == 1,
            30: x[1] + x[2] == 1,
            20: x[0] + x[2] == 1,
        },
        sense=Sense.Maximize,
    )
    inst.deactivate_constraint(20, "relax_middle")
    df = inst.constraints_df(
        kind=ConstraintKind.Regular, include=["label", "parameters", "inactive_reason"]
    )
    assert list(df.index) == [ConstraintID(5), ConstraintID(20), ConstraintID(30)]
    assert _df_snap(df) == snapshot


def test_instance_constraints_df_inactive_reason_active_only_keeps_column(
    snapshot,
):
    """Edge case: `include=("inactive_reason",)` on an active-only view
    (no actually-inactive constraints) must still
    surface the `inactive_reason` column with NA values. Regression
    guard against the column silently disappearing when no row in
    the view carries a reason."""
    df = _instance_all_kinds().constraints_df(
        kind=ConstraintKind.Regular, include=["label", "inactive_reason"]
    )
    assert "inactive_reason" in df.columns
    assert _df_snap(df) == snapshot


def test_instance_constraints_df_one_hot_includes_inactive(snapshot):
    """The all-state query on `kind=ConstraintKind.OneHot` exercises the merge-sort and
    reason-column path on a special-kind constraint. The capability-
    model docs advertise this surface for auditing OneHot →
    regular-constraint conversion (see capability_model.md), so it
    deserves its own snapshot anchor."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={},
        active_one_hot_constraints={
            5: OneHotConstraint(variables=x[:2]),
            7: OneHotConstraint(variables=x[1:]),
        },
        sense=Sense.Maximize,
    )
    instance.convert_one_hot_to_constraint(7)
    df = instance.constraints_df(
        kind=ConstraintKind.OneHot, include=["label", "parameters", "inactive_reason"]
    )
    # Globally id-sorted: active id 5, then inactive id 7.
    assert list(df.index) == [OneHotConstraintID(5), OneHotConstraintID(7)]
    assert _df_snap(df) == snapshot


# ---------------------------------------------------------------------------
# ParametricInstance.constraints_df
# ---------------------------------------------------------------------------


def test_parametric_instance_constraints_df_default(snapshot):
    """`ParametricInstance.constraints_df()` uses the same wide shape as Instance."""
    assert (
        _df_snap(_parametric_instance().constraints_df(kind=ConstraintKind.Regular))
        == snapshot
    )


@pytest.mark.parametrize(
    "kind, expected_name",
    [
        (ConstraintKind.Indicator, "indicator_constraint_id"),
        (ConstraintKind.OneHot, "one_hot_constraint_id"),
        (ConstraintKind.Sos1, "sos1_constraint_id"),
    ],
    ids=str,
)
def test_parametric_instance_constraints_df_special_kinds_empty(
    snapshot, kind, expected_name
):
    """Python `ParametricInstance.from_components` only accepts regular
    constraints, so the special-kind collections are always empty.

    Empty DataFrames must still carry the kind-qualified index name —
    `df.to_string()` happens to render `Index: []` without the name,
    so the assertion on `df.index.name` is the load-bearing check.
    Regression guard for the Wave 2 macro dispatch and for empty-df
    handling in `raw_entries_to_dataframe`."""
    df = _parametric_instance().constraints_df(kind=kind)
    assert df.empty
    assert df.index.name == expected_name
    assert _df_snap(df) == snapshot


# ---------------------------------------------------------------------------
# Solution.constraints_df
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "kind",
    [
        ConstraintKind.Regular,
        ConstraintKind.Indicator,
        ConstraintKind.OneHot,
        ConstraintKind.Sos1,
    ],
    ids=str,
)
def test_solution_constraints_df_kind(snapshot, kind):
    """`Solution.constraints_df(kind=...)` — evaluated stage core columns +
    context, with rows selected by `min_role`."""
    assert _df_snap(_solution_basic().constraints_df(kind=kind)) == snapshot


def test_solution_constraints_df_rejects_legacy_removed_keyword():
    """`Solution.constraints_df` does not accept `removed=`."""
    with pytest.raises(TypeError):
        _solution_basic().constraints_df(kind=ConstraintKind.Regular, removed=True)  # type: ignore[call-arg]


def test_solution_constraints_df_inactive_reason_include(snapshot):
    """Constraints deactivated before evaluation get `inactive_reason` /
    `inactive_reason.{key}` columns when the flag is in `include=`."""
    assert (
        _df_snap(
            _solution_with_inactive().constraints_df(
                kind=ConstraintKind.Regular, include=["label", "inactive_reason"]
            )
        )
        == snapshot
    )


def test_solution_constraints_df_inactive_reason_without_inactive_rows_keeps_column(
    snapshot,
):
    """`include=("inactive_reason",)` on a Solution where no constraint
    was deactivated before evaluation must still surface the
    `inactive_reason` column with NA values. Regression guard for the
    column silently disappearing when no row in the view carries a
    reason."""
    df = _solution_basic().constraints_df(
        kind=ConstraintKind.Regular, include=["label", "inactive_reason"]
    )
    assert "inactive_reason" in df.columns
    assert _df_snap(df) == snapshot


# ---------------------------------------------------------------------------
# SampleSet.constraints_df
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "kind",
    [
        ConstraintKind.Regular,
        ConstraintKind.Indicator,
        ConstraintKind.OneHot,
        ConstraintKind.Sos1,
    ],
    ids=str,
)
def test_sample_set_constraints_df_kind(snapshot, kind):
    """`SampleSet.constraints_df(kind=...)` adds per-sample dynamic columns
    (`value.{sid}`, `feasible.{sid}`, ...) on top of the core schema."""
    assert _df_snap(_sample_set_basic().constraints_df(kind=kind)) == snapshot


def test_sample_set_constraints_df_rejects_legacy_removed_keyword():
    """`SampleSet.constraints_df` does not accept `removed=`."""
    with pytest.raises(TypeError):
        _sample_set_basic().constraints_df(kind=ConstraintKind.Regular, removed=True)  # type: ignore[call-arg]
