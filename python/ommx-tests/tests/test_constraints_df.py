"""Snapshot tests for the unified `constraints_df(kind=, include=, removed=)`
method on Instance / ParametricInstance / Solution / SampleSet.

Wave 2 of the v3-alpha pandas surface collapses 26 per-kind methods into
4 unified `constraints_df` accessors. The `.ambr` snapshots are the
authoritative description of the column / index schema for each host ×
kind × include / removed combination — update via
`pytest --snapshot-update` after a deliberate API change.
"""

from __future__ import annotations

import pandas as pd
import pytest
from ommx.v1 import (
    Constraint,
    DecisionVariable,
    Equality,
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
    """Instance with one constraint of each kind, all carrying metadata."""
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
        constraints={10: c},
        indicator_constraints={
            20: IndicatorConstraint(
                indicator_variable=x[0],
                function=x[1] + x[2] - 1,
                equality=Equality.EqualToZero,
            )
        },
        one_hot_constraints={30: OneHotConstraint(variables=[0, 1, 2])},
        sos1_constraints={40: Sos1Constraint(variables=[0, 1, 2])},
        sense=Instance.MAXIMIZE,
    )


def _instance_with_removed() -> Instance:
    """Instance whose regular constraint at id=10 has been relaxed."""
    inst = _instance_all_kinds()
    inst.relax_constraint(10, "manual_relax", phase="warm-up")
    return inst


def _parametric_instance() -> ParametricInstance:
    """ParametricInstance with one regular constraint that uses a parameter."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    p = Parameter(99, name="p")
    c = x[0] + x[1] + p * x[2] == 1
    assert isinstance(c, Constraint)
    return ParametricInstance.from_components(
        sense=Instance.MAXIMIZE,
        objective=sum(x),
        decision_variables=x,
        constraints={10: c},
        parameters=[p],
    )


def _solution_basic():
    """Solution from `_instance_all_kinds()` evaluated at a feasible point."""
    inst = _instance_all_kinds()
    return inst.evaluate({0: 0.0, 1: 1.0, 2: 0.0})


def _solution_with_removed():
    """Solution where the regular constraint at id=10 was relaxed before evaluation."""
    inst = _instance_with_removed()
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


@pytest.mark.parametrize("kind", ["regular", "indicator", "one_hot", "sos1"])
def test_instance_constraints_df_kind(snapshot, kind):
    """Default `include=` (metadata + parameters); index is `{kind}_constraint_id`."""
    assert _df_snap(_instance_all_kinds().constraints_df(kind=kind)) == snapshot


def test_instance_constraints_df_unknown_kind():
    """Unknown `kind=` raises ValueError."""
    with pytest.raises(ValueError, match="unknown constraint kind"):
        _instance_all_kinds().constraints_df(kind="bogus")


def test_instance_constraints_df_unknown_include_flag():
    """Unknown `include=` flag raises ValueError."""
    with pytest.raises(ValueError, match="unknown include flag"):
        _instance_all_kinds().constraints_df(include=["bogus"])


# ---------------------------------------------------------------------------
# Instance.constraints_df — include= matrix
# ---------------------------------------------------------------------------


def test_instance_constraints_df_include_empty(snapshot):
    """`include=[]` strips metadata + parameters columns; only core columns remain."""
    assert _df_snap(_instance_all_kinds().constraints_df(include=[])) == snapshot


def test_instance_constraints_df_include_metadata_only(snapshot):
    """`include=("metadata",)` keeps name/subscripts/description, drops parameters."""
    assert (
        _df_snap(_instance_all_kinds().constraints_df(include=["metadata"])) == snapshot
    )


def test_instance_constraints_df_include_parameters_only(snapshot):
    """`include=("parameters",)` keeps parameters.{key}, drops name/subscripts/description."""
    assert (
        _df_snap(_instance_all_kinds().constraints_df(include=["parameters"]))
        == snapshot
    )


# ---------------------------------------------------------------------------
# Instance.constraints_df — removed=
# ---------------------------------------------------------------------------


def test_instance_constraints_df_removed_default_excludes_removed(snapshot):
    """Default `removed=False` returns active rows only — relaxed id=10 omitted."""
    assert _df_snap(_instance_with_removed().constraints_df()) == snapshot


def test_instance_constraints_df_removed_true_includes_both(snapshot):
    """`removed=True` returns active + removed; auto-sets `removed_reason` columns."""
    assert _df_snap(_instance_with_removed().constraints_df(removed=True)) == snapshot


def test_instance_constraints_df_removed_true_no_metadata(snapshot):
    """`removed=True` together with `include=[]` keeps removed_reason columns
    (the `removed=True` flag overrides include= for `removed_reason`)."""
    assert (
        _df_snap(_instance_with_removed().constraints_df(include=[], removed=True))
        == snapshot
    )


def test_instance_constraints_df_removed_true_id_sorted(snapshot):
    """`removed=True` returns a globally id-sorted union of active and
    removed rows. With active ids {5, 30} and removed id 20 in the
    middle, the output is [5, 20, 30] — not [5, 30, 20] (active first
    then removed). Regression guard for the merge-sort path; the
    naive `chain(active, removed)` would order rows by section
    rather than by id."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    inst = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={
            5: x[0] + x[1] == 1,
            30: x[1] + x[2] == 1,
            20: x[0] + x[2] == 1,
        },
        sense=Instance.MAXIMIZE,
    )
    inst.relax_constraint(20, "relax_middle")
    df = inst.constraints_df(kind="regular", removed=True)
    assert list(df.index) == [5, 20, 30]
    assert _df_snap(df) == snapshot


def test_instance_constraints_df_removed_reason_active_only_keeps_column(
    snapshot,
):
    """Edge case: `include=("removed_reason",)` on an active-only view
    (no `removed=True`, no actually-removed constraints) must still
    surface the `removed_reason` column with NA values. Regression
    guard against the column silently disappearing when no row in
    the view carries a reason."""
    df = _instance_all_kinds().constraints_df(
        kind="regular", include=["metadata", "removed_reason"]
    )
    assert "removed_reason" in df.columns
    assert _df_snap(df) == snapshot


# ---------------------------------------------------------------------------
# ParametricInstance.constraints_df
# ---------------------------------------------------------------------------


def test_parametric_instance_constraints_df_default(snapshot):
    """`ParametricInstance.constraints_df()` uses the same wide shape as Instance."""
    assert _df_snap(_parametric_instance().constraints_df()) == snapshot


@pytest.mark.parametrize("kind", ["indicator", "one_hot", "sos1"])
def test_parametric_instance_constraints_df_special_kinds_empty(snapshot, kind):
    """Python `ParametricInstance.from_components` only accepts regular
    constraints, so the special-kind collections are always empty —
    but the dispatch path must still return a DataFrame with the
    correct kind-qualified index name.

    Regression guard for the Wave 2 macro dispatch on the three
    special-kind ParametricInstance accessors that the public Python
    surface cannot populate."""
    assert _df_snap(_parametric_instance().constraints_df(kind=kind)) == snapshot


# ---------------------------------------------------------------------------
# Solution.constraints_df
# ---------------------------------------------------------------------------


@pytest.mark.parametrize("kind", ["regular", "indicator", "one_hot", "sos1"])
def test_solution_constraints_df_kind(snapshot, kind):
    """`Solution.constraints_df(kind=...)` — evaluated stage core columns +
    metadata. No `removed=` parameter at this stage."""
    assert _df_snap(_solution_basic().constraints_df(kind=kind)) == snapshot


def test_solution_constraints_df_no_removed_parameter():
    """`Solution.constraints_df` does not accept `removed=`."""
    with pytest.raises(TypeError):
        _solution_basic().constraints_df(removed=True)  # type: ignore[call-arg]


def test_solution_constraints_df_removed_reason_include(snapshot):
    """Constraints removed before evaluation get `removed_reason` /
    `removed_reason.{key}` columns when the flag is in `include=`."""
    assert (
        _df_snap(
            _solution_with_removed().constraints_df(
                include=["metadata", "removed_reason"]
            )
        )
        == snapshot
    )


def test_solution_constraints_df_removed_reason_no_removals_keeps_column(snapshot):
    """`include=("removed_reason",)` on a Solution where no constraint
    was removed before evaluation must still surface the
    `removed_reason` column with NA values. Regression guard for the
    column silently disappearing when no row in the view carries a
    reason."""
    df = _solution_basic().constraints_df(
        kind="regular", include=["metadata", "removed_reason"]
    )
    assert "removed_reason" in df.columns
    assert _df_snap(df) == snapshot


# ---------------------------------------------------------------------------
# SampleSet.constraints_df
# ---------------------------------------------------------------------------


@pytest.mark.parametrize("kind", ["regular", "indicator", "one_hot", "sos1"])
def test_sample_set_constraints_df_kind(snapshot, kind):
    """`SampleSet.constraints_df(kind=...)` adds per-sample dynamic columns
    (`value.{sid}`, `feasible.{sid}`, ...) on top of the core schema."""
    assert _df_snap(_sample_set_basic().constraints_df(kind=kind)) == snapshot


def test_sample_set_constraints_df_no_removed_parameter():
    """`SampleSet.constraints_df` does not accept `removed=`."""
    with pytest.raises(TypeError):
        _sample_set_basic().constraints_df(removed=True)  # type: ignore[call-arg]
