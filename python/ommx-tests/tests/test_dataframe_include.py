"""Tests for the `include=` parameter on wide `*_df` methods.

Default `include` matches the v2-equivalent wide shape (`("metadata",
"parameters")`); `include=[]` drops both metadata and parameter columns;
`include=["metadata"]` and `include=["parameters"]` keep only the named
family.

Most assertions are snapshot-based (syrupy) — the `.ambr` file is the
authoritative description of each method's column shape under each
`include=` setting. Update via `pytest --snapshot-update` after a
deliberate API change.
"""

from __future__ import annotations

import pandas as pd
import pytest
from ommx.v1 import (
    Constraint,
    DecisionVariable,
    Instance,
)


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


def _df_snap(df: pd.DataFrame) -> str:
    """Deterministic, snapshot-friendly rendering of a DataFrame."""
    return df.to_string(na_rep="<NA>")


# ---------------------------------------------------------------------------
# decision_variables_df — DV has both metadata and parameters columns
# ---------------------------------------------------------------------------


def test_decision_variables_df_default(snapshot):
    """Default include=("metadata","parameters") — both column families on."""
    assert _df_snap(_build_instance().decision_variables_df()) == snapshot


def test_decision_variables_df_include_empty(snapshot):
    """include=[] — metadata + parameters columns dropped, core columns remain."""
    assert _df_snap(_build_instance().decision_variables_df(include=[])) == snapshot


def test_decision_variables_df_include_metadata_only(snapshot):
    """include=["metadata"] — name/subscripts/description kept, parameters.* dropped."""
    assert (
        _df_snap(_build_instance().decision_variables_df(include=["metadata"]))
        == snapshot
    )


def test_decision_variables_df_include_parameters_only(snapshot):
    """include=["parameters"] — parameters.* kept, name/subscripts/description dropped."""
    assert (
        _df_snap(_build_instance().decision_variables_df(include=["parameters"]))
        == snapshot
    )


# ---------------------------------------------------------------------------
# constraints_df — Constraint has only metadata columns; parameters family
# is currently not emitted by the wide *_df, so include=("parameters",) is
# a no-op there. (The constraint's own `parameters` map is exposed only
# via the long-format `constraint_parameters_df` sidecar.)
# ---------------------------------------------------------------------------


def test_constraints_df_default(snapshot):
    assert _df_snap(_build_instance().constraints_df()) == snapshot


def test_constraints_df_include_empty(snapshot):
    """include=[] drops metadata columns; equality / function_type / used_ids stay."""
    assert _df_snap(_build_instance().constraints_df(include=[])) == snapshot


# ---------------------------------------------------------------------------
# Solution / SampleSet propagate the same shape
# ---------------------------------------------------------------------------


def test_solution_decision_variables_df_include_empty(snapshot):
    instance = _build_instance()
    sol = instance.evaluate({0: 1, 1: 0, 2: 0})
    assert _df_snap(sol.decision_variables_df(include=[])) == snapshot


def test_sample_set_decision_variables_df_include_empty(snapshot):
    instance = _build_instance()
    ss = instance.evaluate_samples({0: {0: 1, 1: 0, 2: 0}, 1: {0: 0, 1: 1, 2: 0}})
    assert _df_snap(ss.decision_variables_df(include=[])) == snapshot


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------


def test_unknown_include_flag_raises_value_error():
    instance = _build_instance()
    with pytest.raises(ValueError):
        instance.decision_variables_df(include=["bogus"])
