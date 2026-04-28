"""Tests for the long-format / id-indexed sidecar DataFrames.

The 6 sidecar accessors on Instance / ParametricInstance / Solution /
SampleSet are derived views over the SoA metadata stores. They expose
metadata in shapes that the wide `*_df` cannot represent without
column-space explosion (provenance chains, per-id parameter maps with
arbitrary keys).

Most assertions are snapshot-based (syrupy) — the `.ambr` file is the
authoritative description of each accessor's column / index schema.
Update via `pytest --snapshot-update` after a deliberate API change.
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
    Sos1Constraint,
)


def _instance_with_metadata() -> Instance:
    """Instance carrying metadata + parameters on regular constraints + variables.

    Variable 0 has 2 parameters; variables 1, 2 have none. The single
    regular constraint has 2 parameters and a 2-entry subscripts list.
    """
    x = [
        DecisionVariable.binary(
            0,
            name="x0",
            subscripts=[0],
            description="primary slot",
            parameters={"role": "primary", "shard": "a"},
        ),
        DecisionVariable.binary(1, name="x1", subscripts=[1]),
        DecisionVariable.binary(2, name="x2", subscripts=[2]),
    ]
    c = (x[0] + x[1] + x[2] == 1).set_name("balance").set_subscripts([0, 1])
    c = c.set_parameters({"region": "us-east", "tier": "gold"})
    c = c.set_description("demand-balance row")
    assert isinstance(c, Constraint)
    return Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={10: c},
        sense=Instance.MAXIMIZE,
    )


def _df_snap(df: pd.DataFrame) -> str:
    """Deterministic, snapshot-friendly rendering of a DataFrame."""
    return df.to_string(na_rep="<NA>")


# ---------------------------------------------------------------------------
# variable_metadata_df / variable_parameters_df
# ---------------------------------------------------------------------------


def test_variable_metadata_df(snapshot):
    """id-indexed wide; columns name / subscripts / description; index = variable_id."""
    assert _df_snap(_instance_with_metadata().variable_metadata_df()) == snapshot


def test_variable_parameters_df(snapshot):
    """Long format. Variable 0 has 2 parameters, 1 and 2 have none → 2 rows."""
    assert _df_snap(_instance_with_metadata().variable_parameters_df()) == snapshot


# ---------------------------------------------------------------------------
# constraint_metadata_df / constraint_parameters_df with kind dispatch
# ---------------------------------------------------------------------------


def test_constraint_metadata_df_default_kind_is_regular(snapshot):
    """No kind= argument → kind="regular"; index = regular_constraint_id."""
    assert _df_snap(_instance_with_metadata().constraint_metadata_df()) == snapshot


def test_constraint_parameters_df(snapshot):
    """Long format with regular_constraint_id, key, value columns."""
    assert _df_snap(_instance_with_metadata().constraint_parameters_df()) == snapshot


def test_unknown_kind_raises_value_error():
    instance = _instance_with_metadata()
    with pytest.raises(ValueError):
        instance.constraint_metadata_df(kind="bogus")  # type: ignore[arg-type]


def test_indicator_kind_metadata_df(snapshot):
    """Each constraint family's id column carries a kind-qualified index name."""
    assert (
        _df_snap(_special_instance().constraint_metadata_df(kind="indicator"))
        == snapshot
    )


def test_one_hot_kind_metadata_df(snapshot):
    assert (
        _df_snap(_special_instance().constraint_metadata_df(kind="one_hot")) == snapshot
    )


def test_sos1_kind_metadata_df(snapshot):
    assert _df_snap(_special_instance().constraint_metadata_df(kind="sos1")) == snapshot


def _special_instance() -> Instance:
    """Instance with one of each special constraint kind."""
    x = [DecisionVariable.binary(i) for i in range(4)]
    return Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        indicator_constraints={
            5: IndicatorConstraint(
                indicator_variable=x[0],
                function=x[1] + x[2] - 1,
                equality=Equality.EqualToZero,
            )
        },
        one_hot_constraints={6: OneHotConstraint(variables=[1, 2, 3])},
        sos1_constraints={7: Sos1Constraint(variables=[0, 1, 2, 3])},
        sense=Instance.MAXIMIZE,
    )


# ---------------------------------------------------------------------------
# constraint_provenance_df is empty on directly-authored constraints
# ---------------------------------------------------------------------------


def test_provenance_empty_when_no_chain(snapshot):
    """Directly-authored constraints have no provenance chain."""
    assert _df_snap(_instance_with_metadata().constraint_provenance_df()) == snapshot


def test_provenance_after_one_hot_conversion(snapshot):
    """`convert_one_hot_to_constraint` promotes a OneHot row into a regular
    constraint; the new constraint records `OneHotConstraint(7)` in its
    provenance chain."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={7: OneHotConstraint(variables=[0, 1, 2])},
        sense=Instance.MINIMIZE,
    )
    instance.convert_one_hot_to_constraint(7)
    assert _df_snap(instance.constraint_provenance_df()) == snapshot


# ---------------------------------------------------------------------------
# constraint_removed_reasons_df
# ---------------------------------------------------------------------------


def test_removed_reasons_df_after_relax(snapshot):
    """relax_constraint with no extra parameters → 1 row, key/value = NA."""
    instance = _instance_with_metadata()
    instance.relax_constraint(10, "test_reason")
    assert _df_snap(instance.constraint_removed_reasons_df()) == snapshot


def test_removed_reasons_df_with_parameters_after_one_hot_conversion(snapshot):
    """`convert_one_hot_to_constraint` records the conversion reason with a
    `constraint_id` parameter — verifies the long-format expansion of the
    parameter map."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={7: OneHotConstraint(variables=[0, 1, 2])},
        sense=Instance.MINIMIZE,
    )
    instance.convert_one_hot_to_constraint(7)
    assert _df_snap(instance.constraint_removed_reasons_df(kind="one_hot")) == snapshot


# ---------------------------------------------------------------------------
# Solution / SampleSet expose the same surface; the metadata stores are
# stage-independent so the rendered DataFrame is byte-identical to
# Instance's.
# ---------------------------------------------------------------------------


def test_solution_constraint_metadata_df_matches_instance():
    instance = _instance_with_metadata()
    sol = instance.evaluate({0: 1, 1: 0, 2: 0})
    pd.testing.assert_frame_equal(
        instance.constraint_metadata_df(), sol.constraint_metadata_df()
    )


def test_sample_set_variable_metadata_df_matches_instance():
    instance = _instance_with_metadata()
    ss = instance.evaluate_samples({0: {0: 1, 1: 0, 2: 0}})
    pd.testing.assert_frame_equal(
        instance.variable_metadata_df(), ss.variable_metadata_df()
    )


# ---------------------------------------------------------------------------
# Regression: Solution/SampleSet sidecars must NOT duplicate rows for
# removed constraints. EvaluatedCollection.inner() / SampledCollection.inner()
# already include removed ids, so chaining `.removed_reasons().keys()` would
# double-count them.
# ---------------------------------------------------------------------------


def _instance_with_relaxed_constraint() -> Instance:
    """Two regular constraints, one of which is relaxed (moved to removed)."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={
            10: (x[0] + x[1] == 1).set_name("balance").set_parameters({"k": "v"}),
            11: (x[1] + x[2] <= 1).set_name("cap"),
        },
        sense=Instance.MAXIMIZE,
    )
    instance.relax_constraint(10, "test_reason")
    return instance


def test_solution_sidecar_no_duplicate_rows_for_removed():
    """The relaxed id 10 lives in both inner() and removed_reasons() — the
    sidecar must emit one row per id, not two."""
    instance = _instance_with_relaxed_constraint()
    sol = instance.evaluate({0: 1, 1: 0, 2: 0})

    meta = sol.constraint_metadata_df()
    assert sorted(meta.index.tolist()) == [10, 11]

    params = sol.constraint_parameters_df()
    rows = list(zip(params["regular_constraint_id"], params["key"]))
    assert rows == [(10, "k")]  # exactly one row, not duplicated


def test_sample_set_sidecar_no_duplicate_rows_for_removed():
    instance = _instance_with_relaxed_constraint()
    ss = instance.evaluate_samples({0: {0: 1, 1: 0, 2: 0}})

    meta = ss.constraint_metadata_df()
    assert sorted(meta.index.tolist()) == [10, 11]

    params = ss.constraint_parameters_df()
    rows = list(zip(params["regular_constraint_id"], params["key"]))
    assert rows == [(10, "k")]


# ---------------------------------------------------------------------------
# Regression: include=["parameters"] on Solution/SampleSet decision variables
# now emits parameters.{key} columns (used to silently drop them).
# ---------------------------------------------------------------------------


def test_solution_decision_variables_df_emits_parameter_columns():
    instance = _instance_with_metadata()
    sol = instance.evaluate({0: 1, 1: 0, 2: 0})
    df = sol.decision_variables_df(include=["parameters"])
    assert "parameters.role" in df.columns
    assert "parameters.shard" in df.columns


def test_sample_set_decision_variables_df_emits_parameter_columns():
    instance = _instance_with_metadata()
    ss = instance.evaluate_samples({0: {0: 1, 1: 0, 2: 0}})
    df = ss.decision_variables_df(include=["parameters"])
    assert "parameters.role" in df.columns
    assert "parameters.shard" in df.columns
