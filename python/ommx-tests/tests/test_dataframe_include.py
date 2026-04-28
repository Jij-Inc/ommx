"""Tests for the `include=` parameter on wide `*_df` methods.

Default `include` matches the v2-equivalent wide shape (`("metadata",
"parameters")`); `include=()` drops both metadata and parameter columns;
`include=("metadata",)` and `include=("parameters",)` keep only the named
family.
"""

from __future__ import annotations

import pytest
from ommx.v1 import (
    DecisionVariable,
    Instance,
    Constraint,
)


METADATA_COLS = {"name", "subscripts", "description"}


def _build_instance() -> Instance:
    """Instance with metadata + parameters on decision variables and constraints."""
    x = [
        DecisionVariable.binary(
            i,
            name=f"x{i}",
            subscripts=[i],
            description=f"variable {i}",
            parameters={"role": "primary", "shard": str(i)},
        )
        for i in range(3)
    ]
    c = (x[0] + x[1] + x[2] == 1).set_name("balance")
    assert isinstance(c, Constraint)
    return Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={10: c},
        sense=Instance.MAXIMIZE,
    )


# ---------------------------------------------------------------------------
# decision_variables_df — DV has both metadata and parameters columns
# ---------------------------------------------------------------------------


def test_decision_variables_df_default_includes_both():
    instance = _build_instance()
    df = instance.decision_variables_df()
    assert METADATA_COLS.issubset(df.columns)
    assert "parameters.role" in df.columns
    assert "parameters.shard" in df.columns


def test_decision_variables_df_include_empty_drops_both():
    instance = _build_instance()
    df = instance.decision_variables_df(include=[])
    assert METADATA_COLS.isdisjoint(df.columns)
    assert not any(c.startswith("parameters.") for c in df.columns)


def test_decision_variables_df_include_metadata_only():
    instance = _build_instance()
    df = instance.decision_variables_df(include=["metadata"])
    assert METADATA_COLS.issubset(df.columns)
    assert not any(c.startswith("parameters.") for c in df.columns)


def test_decision_variables_df_include_parameters_only():
    instance = _build_instance()
    df = instance.decision_variables_df(include=["parameters"])
    assert METADATA_COLS.isdisjoint(df.columns)
    assert "parameters.role" in df.columns
    assert "parameters.shard" in df.columns


# ---------------------------------------------------------------------------
# constraints_df — Constraint has only metadata columns; parameters family
# is currently not emitted, so include=("parameters",) is a no-op.
# ---------------------------------------------------------------------------


def test_constraints_df_default_emits_metadata():
    instance = _build_instance()
    df = instance.constraints_df()
    assert METADATA_COLS.issubset(df.columns)


def test_constraints_df_include_empty_drops_metadata():
    instance = _build_instance()
    df = instance.constraints_df(include=[])
    assert METADATA_COLS.isdisjoint(df.columns)
    # core columns are still present
    assert "equality" in df.columns


# ---------------------------------------------------------------------------
# Solution / SampleSet propagate the same shape
# ---------------------------------------------------------------------------


def test_solution_decision_variables_df_include_empty():
    instance = _build_instance()
    sol = instance.evaluate({0: 1, 1: 0, 2: 0})
    df = sol.decision_variables_df(include=[])
    assert METADATA_COLS.isdisjoint(df.columns)
    assert "value" in df.columns


def test_sample_set_decision_variables_df_include_empty():
    instance = _build_instance()
    ss = instance.evaluate_samples({0: {0: 1, 1: 0, 2: 0}, 1: {0: 0, 1: 1, 2: 0}})
    df = ss.decision_variables_df(include=[])
    assert METADATA_COLS.isdisjoint(df.columns)
    # per-sample value columns remain
    assert 0 in df.columns
    assert 1 in df.columns


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------


def test_unknown_include_flag_raises_value_error():
    instance = _build_instance()
    with pytest.raises(ValueError):
        instance.decision_variables_df(include=["bogus"])
