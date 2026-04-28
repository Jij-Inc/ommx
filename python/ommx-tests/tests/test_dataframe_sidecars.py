"""Tests for the long-format / id-indexed sidecar DataFrames.

The 6 sidecar accessors on Instance / ParametricInstance / Solution /
SampleSet are derived views over the SoA metadata stores. They expose
metadata in shapes that the wide `*_df` cannot represent without
column-space explosion (provenance chains, per-id parameter maps with
arbitrary keys).
"""

from __future__ import annotations

import math
import pytest
import pandas as pd
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


# ---------------------------------------------------------------------------
# variable_metadata_df / variable_parameters_df
# ---------------------------------------------------------------------------


def test_variable_metadata_df_is_id_indexed_with_columns():
    df = _instance_with_metadata().variable_metadata_df()
    assert df.index.name == "variable_id"
    assert list(df.index) == [0, 1, 2]
    assert {"name", "subscripts", "description"} <= set(df.columns)
    assert df.loc[0, "name"] == "x0"
    assert df.loc[0, "description"] == "primary slot"
    # name is set on every variable in the fixture but description only on 0.
    assert pd.isna(df.loc[1, "description"])


def test_variable_parameters_df_long_format_only_emits_present_keys():
    df = _instance_with_metadata().variable_parameters_df()
    assert list(df.columns) == ["variable_id", "key", "value"]
    # Variable 0 has 2 parameters, 1 and 2 have none.
    rows = {
        (int(vid), key): val
        for vid, key, val in zip(df["variable_id"], df["key"], df["value"])
    }
    assert rows == {(0, "role"): "primary", (0, "shard"): "a"}


# ---------------------------------------------------------------------------
# constraint_metadata_df / constraint_parameters_df with kind dispatch
# ---------------------------------------------------------------------------


def test_constraint_metadata_df_default_kind_is_regular():
    df = _instance_with_metadata().constraint_metadata_df()
    assert df.index.name == "regular_constraint_id"
    assert list(df.index) == [10]
    assert df.loc[10, "name"] == "balance"
    assert df.loc[10, "subscripts"] == [0, 1]
    assert df.loc[10, "description"] == "demand-balance row"


def test_constraint_parameters_df_long_format():
    df = _instance_with_metadata().constraint_parameters_df()
    assert list(df.columns) == ["regular_constraint_id", "key", "value"]
    rows = {
        (int(cid), key): val
        for cid, key, val in zip(df["regular_constraint_id"], df["key"], df["value"])
    }
    assert rows == {(10, "region"): "us-east", (10, "tier"): "gold"}


def test_unknown_kind_raises_value_error():
    instance = _instance_with_metadata()
    with pytest.raises(ValueError):
        instance.constraint_metadata_df(kind="bogus")


def test_each_kind_uses_qualified_index_name():
    """Each constraint family's id column carries a kind-qualified name so
    cross-kind joins are visible. Verifies the column / index naming on
    indicator / one_hot / sos1 dispatch paths."""
    x = [DecisionVariable.binary(i) for i in range(4)]
    instance = Instance.from_components(
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
    assert (
        instance.constraint_metadata_df(kind="indicator").index.name
        == "indicator_constraint_id"
    )
    assert (
        instance.constraint_metadata_df(kind="one_hot").index.name
        == "one_hot_constraint_id"
    )
    assert (
        instance.constraint_metadata_df(kind="sos1").index.name == "sos1_constraint_id"
    )


# ---------------------------------------------------------------------------
# constraint_provenance_df is empty on directly-authored constraints
# ---------------------------------------------------------------------------


def test_provenance_empty_when_no_chain():
    df = _instance_with_metadata().constraint_provenance_df()
    assert df.empty or list(df.columns) == [
        "regular_constraint_id",
        "step",
        "source_kind",
        "source_id",
    ]


def test_provenance_after_one_hot_conversion():
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
    new_id = instance.convert_one_hot_to_constraint(7)
    df = instance.constraint_provenance_df()
    rows = [
        (int(cid), int(step), src_kind, int(src_id))
        for cid, step, src_kind, src_id in zip(
            df["regular_constraint_id"],
            df["step"],
            df["source_kind"],
            df["source_id"],
        )
    ]
    assert (int(new_id), 0, "OneHotConstraint", 7) in rows


# ---------------------------------------------------------------------------
# constraint_removed_reasons_df
# ---------------------------------------------------------------------------


def test_removed_reasons_df_after_relax():
    instance = _instance_with_metadata()
    instance.relax_constraint(10, "test_reason")
    df = instance.constraint_removed_reasons_df()
    assert list(df.columns) == [
        "regular_constraint_id",
        "reason",
        "key",
        "value",
    ]
    # The relax_constraint call provided no extra parameters → 1 row with
    # NA key/value.
    assert len(df) == 1
    assert int(df["regular_constraint_id"].iloc[0]) == 10
    assert df["reason"].iloc[0] == "test_reason"
    key0 = df["key"].iloc[0]
    assert (
        key0 is None or (isinstance(key0, float) and math.isnan(key0)) or pd.isna(key0)
    )


def test_removed_reasons_df_with_parameters_after_one_hot_conversion():
    """`convert_one_hot_to_constraint` records the conversion reason with a
    `constraint_ids` parameter — verifies the long-format expansion of the
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
    df = instance.constraint_removed_reasons_df(kind="one_hot")
    assert len(df) == 1
    assert int(df["one_hot_constraint_id"].iloc[0]) == 7
    assert df["reason"].iloc[0] == "ommx.Instance.convert_one_hot_to_constraint"
    # The reason carries a single `constraint_id` parameter naming the
    # promoted regular constraint id.
    assert df["key"].iloc[0] == "constraint_id"
    assert isinstance(df["value"].iloc[0], str)


# ---------------------------------------------------------------------------
# Solution / SampleSet expose the same surface; sanity-check the parity.
# ---------------------------------------------------------------------------


def test_solution_constraint_metadata_df_matches_instance():
    instance = _instance_with_metadata()
    sol = instance.evaluate({0: 1, 1: 0, 2: 0})
    df_inst = instance.constraint_metadata_df()
    df_sol = sol.constraint_metadata_df()
    pd.testing.assert_frame_equal(df_inst, df_sol)


def test_sample_set_variable_metadata_df_matches_instance():
    instance = _instance_with_metadata()
    ss = instance.evaluate_samples({0: {0: 1, 1: 0, 2: 0}})
    df_inst = instance.variable_metadata_df()
    df_ss = ss.variable_metadata_df()
    pd.testing.assert_frame_equal(df_inst, df_ss)
