"""Tests for OneHotConstraint and Sos1Constraint as first-class constraint types."""

import pandas as pd
import pytest
from ommx.v1 import Instance, DecisionVariable, OneHotConstraint, Sos1Constraint


def _df_snap(df: pd.DataFrame) -> str:
    """Deterministic, snapshot-friendly rendering of a DataFrame."""
    return df.to_string(na_rep="<NA>")


def test_one_hot_constraint_from_components():
    """Test creating an instance with OneHotConstraint via from_components."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    oh = OneHotConstraint(variables=[1, 2, 3])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints={},
        one_hot_constraints={10: oh},
        sense=Instance.MINIMIZE,
    )

    assert len(instance.one_hot_constraints) == 1
    assert 10 in instance.one_hot_constraints
    assert instance.one_hot_constraints[10].variables == [1, 2, 3]


def test_sos1_constraint_from_components():
    """Test creating an instance with Sos1Constraint via from_components."""
    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    objective = sum(x)

    sos1 = Sos1Constraint(variables=[1, 2, 3])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints={},
        sos1_constraints={20: sos1},
        sense=Instance.MINIMIZE,
    )

    assert len(instance.sos1_constraints) == 1
    assert 20 in instance.sos1_constraints
    assert instance.sos1_constraints[20].variables == [1, 2, 3]


def test_one_hot_variable_not_defined():
    """Test that OneHotConstraint with undefined variable ID fails."""
    x = [DecisionVariable.binary(1)]
    objective = x[0]

    oh = OneHotConstraint(variables=[1, 999])  # 999 doesn't exist

    with pytest.raises(RuntimeError):
        Instance.from_components(
            decision_variables=x,
            objective=objective,
            constraints={},
            one_hot_constraints={10: oh},
            sense=Instance.MINIMIZE,
        )


def test_sos1_variable_not_defined():
    """Test that Sos1Constraint with undefined variable ID fails."""
    x = [DecisionVariable.continuous(1, lower=0, upper=10)]
    objective = x[0]

    sos1 = Sos1Constraint(variables=[1, 999])  # 999 doesn't exist

    with pytest.raises(RuntimeError):
        Instance.from_components(
            decision_variables=x,
            objective=objective,
            constraints={},
            sos1_constraints={20: sos1},
            sense=Instance.MINIMIZE,
        )


def test_one_hot_variable_not_binary():
    """Test that OneHotConstraint with non-binary variable fails."""
    x = [
        DecisionVariable.binary(1),
        DecisionVariable.continuous(2, lower=0, upper=1),  # not binary
    ]
    objective = sum(x)

    oh = OneHotConstraint(variables=[1, 2])

    with pytest.raises(RuntimeError, match="One-hot variable.*must be binary"):
        Instance.from_components(
            decision_variables=x,
            objective=objective,
            constraints={},
            one_hot_constraints={10: oh},
            sense=Instance.MINIMIZE,
        )


def test_serialize_not_yet_supported():
    """Serialization of OneHot/SOS1 constraints to v1 proto is not yet supported."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=[1, 2, 3])},
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(BaseException):
        instance.to_bytes()


def test_both_one_hot_and_sos1():
    """Test instance with both OneHot and SOS1 constraints."""
    x = [DecisionVariable.binary(i) for i in range(1, 6)]
    objective = sum(x)

    oh = OneHotConstraint(variables=[1, 2, 3])
    sos1 = Sos1Constraint(variables=[3, 4, 5])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints={},
        one_hot_constraints={10: oh},
        sos1_constraints={20: sos1},
        sense=Instance.MINIMIZE,
    )

    assert len(instance.one_hot_constraints) == 1
    assert len(instance.sos1_constraints) == 1


def test_evaluate_with_one_hot_feasible():
    """Test that evaluation with OneHot constraints checks feasibility."""
    from ommx.v1 import State

    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    oh = OneHotConstraint(variables=[1, 2, 3])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints={},
        one_hot_constraints={10: oh},
        sense=Instance.MINIMIZE,
    )

    # x1=0, x2=1, x3=0 → feasible (exactly one is 1)
    state = State({1: 0.0, 2: 1.0, 3: 0.0})
    solution = instance.evaluate(state)
    assert solution.feasible


def test_evaluate_with_one_hot_infeasible():
    """Test that evaluation with OneHot constraints detects infeasibility."""
    from ommx.v1 import State

    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    oh = OneHotConstraint(variables=[1, 2, 3])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints={},
        one_hot_constraints={10: oh},
        sense=Instance.MINIMIZE,
    )

    # x1=1, x2=1, x3=0 → infeasible (two are 1)
    state = State({1: 1.0, 2: 1.0, 3: 0.0})
    solution = instance.evaluate(state)
    assert not solution.feasible


def test_evaluate_with_sos1_feasible():
    """Test that evaluation with SOS1 constraints checks feasibility."""
    from ommx.v1 import State

    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    objective = sum(x)

    sos1 = Sos1Constraint(variables=[1, 2, 3])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints={},
        sos1_constraints={20: sos1},
        sense=Instance.MINIMIZE,
    )

    # x1=0, x2=5, x3=0 → feasible (at most one non-zero)
    state = State({1: 0.0, 2: 5.0, 3: 0.0})
    solution = instance.evaluate(state)
    assert solution.feasible

    # All zeros → also feasible for SOS1
    state_zeros = State({1: 0.0, 2: 0.0, 3: 0.0})
    solution_zeros = instance.evaluate(state_zeros)
    assert solution_zeros.feasible


def test_convert_sos1_with_integer_variables_emits_bigm_pair():
    """Non-binary SOS1 variables get a fresh binary indicator plus upper and lower Big-M."""
    x = [DecisionVariable.integer(i, lower=-2, upper=3) for i in range(1, 3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sos1_constraints={10: Sos1Constraint(variables=[1, 2])},
        sense=Instance.MINIMIZE,
    )

    before_var_ids = {dv.id for dv in instance.decision_variables}
    new_ids = instance.convert_sos1_to_constraints(10)

    # Two integer vars with [-2, 3]: upper + lower per var, plus cardinality = 5 constraints.
    assert len(new_ids) == 5
    # Two fresh binary indicators were introduced.
    after_vars = {dv.id: dv for dv in instance.decision_variables}
    new_var_ids = set(after_vars) - before_var_ids
    assert len(new_var_ids) == 2
    for v_id in new_var_ids:
        assert after_vars[v_id].name == "ommx.sos1_indicator"
    # Original SOS1 is retained on the removed side with our reason string.
    removed = instance.removed_sos1_constraints[10]
    assert removed.removed_reason == "ommx.Instance.convert_sos1_to_constraints"
    # `constraint_ids` parameter lists all 5 new IDs in insertion order.
    assert removed.removed_reason_parameters["constraint_ids"] == ",".join(
        str(i) for i in new_ids
    )


def test_convert_sos1_rejects_domain_excluding_zero():
    """SOS1 over x with bound [1, 3] cannot be Big-M converted: y=0 ⇒ x=0 is infeasible."""
    x = [DecisionVariable.integer(1, lower=1, upper=3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints={},
        sos1_constraints={10: Sos1Constraint(variables=[1])},
        sense=Instance.MINIMIZE,
    )
    before_var_ids = {dv.id for dv in instance.decision_variables}

    with pytest.raises(RuntimeError, match="excludes 0"):
        instance.convert_sos1_to_constraints(10)

    # Instance unchanged on error.
    assert {dv.id for dv in instance.decision_variables} == before_var_ids
    assert 10 in instance.sos1_constraints


def test_sos1_constraints_df_roundtrips_removed_metadata(snapshot):
    """`constraints_df(kind="sos1", removed=True)` surfaces reason + constraint_ids
    parameter as columns on removed rows. Active is empty after conversion."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sos1_constraints={7: Sos1Constraint(variables=[0, 1, 2])},
        sense=Instance.MINIMIZE,
    )
    instance.convert_sos1_to_constraints(7)

    assert instance.constraints_df(kind="sos1").empty
    assert _df_snap(instance.constraints_df(kind="sos1", removed=True)) == snapshot


def test_evaluate_with_sos1_infeasible():
    """Test that evaluation with SOS1 constraints detects infeasibility."""
    from ommx.v1 import State

    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    objective = sum(x)

    sos1 = Sos1Constraint(variables=[1, 2, 3])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints={},
        sos1_constraints={20: sos1},
        sense=Instance.MINIMIZE,
    )

    # x1=1, x2=2, x3=0 → infeasible (two non-zero)
    state = State({1: 1.0, 2: 2.0, 3: 0.0})
    solution = instance.evaluate(state)
    assert not solution.feasible


@pytest.fixture
def _solution_one_hot_feasible():
    """One-hot constraint with a feasible solution (active_variable=2)."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=[1, 2, 3])},
        sense=Instance.MAXIMIZE,
    )
    return instance.evaluate({1: 0.0, 2: 1.0, 3: 0.0})


@pytest.fixture
def _solution_one_hot_infeasible():
    """One-hot constraint with an infeasible solution (active_variable=NA)."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=[1, 2, 3])},
        sense=Instance.MAXIMIZE,
    )
    return instance.evaluate({1: 1.0, 2: 1.0, 3: 0.0})


def test_solution_one_hot_constraints_df_feasible(snapshot, _solution_one_hot_feasible):
    """`Solution.constraints_df(kind="one_hot")` for feasible: feasible=True,
    active_variable=2."""
    assert (
        _df_snap(_solution_one_hot_feasible.constraints_df(kind="one_hot")) == snapshot
    )


def test_solution_one_hot_constraints_df_infeasible(
    snapshot, _solution_one_hot_infeasible
):
    """`Solution.constraints_df(kind="one_hot")` for infeasible: feasible=False,
    active_variable=NA."""
    assert (
        _df_snap(_solution_one_hot_infeasible.constraints_df(kind="one_hot"))
        == snapshot
    )


@pytest.fixture
def _solution_sos1_one_active():
    """SOS1 with exactly one nonzero variable → feasible, active_variable=2."""
    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sos1_constraints={20: Sos1Constraint(variables=[1, 2, 3])},
        sense=Instance.MINIMIZE,
    )
    return instance.evaluate({1: 0.0, 2: 5.0, 3: 0.0})


@pytest.fixture
def _solution_sos1_all_zero():
    """SOS1 with all zero → feasible (≤1 nonzero), active_variable=NA."""
    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sos1_constraints={20: Sos1Constraint(variables=[1, 2, 3])},
        sense=Instance.MINIMIZE,
    )
    return instance.evaluate({1: 0.0, 2: 0.0, 3: 0.0})


def test_solution_sos1_constraints_df_one_active(snapshot, _solution_sos1_one_active):
    """`Solution.constraints_df(kind="sos1")` with one nonzero: feasible=True,
    active_variable=2."""
    assert _df_snap(_solution_sos1_one_active.constraints_df(kind="sos1")) == snapshot


def test_solution_sos1_constraints_df_all_zero(snapshot, _solution_sos1_all_zero):
    """`Solution.constraints_df(kind="sos1")` with all-zero: feasible=True,
    active_variable=NA."""
    assert _df_snap(_solution_sos1_all_zero.constraints_df(kind="sos1")) == snapshot


def test_solution_one_hot_removed_reasons_df_after_conversion(snapshot):
    """`Solution.constraints_df(kind="one_hot", include=(..., "removed_reason"))`
    after `convert_one_hot_to_constraint` carries the conversion reason."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={7: OneHotConstraint(variables=[0, 1, 2])},
        sense=Instance.MINIMIZE,
    )
    instance.convert_one_hot_to_constraint(7)
    sol = instance.evaluate({0: 1.0, 1: 0.0, 2: 0.0})
    assert (
        _df_snap(
            sol.constraints_df(kind="one_hot", include=["metadata", "removed_reason"])
        )
        == snapshot
    )


def test_solution_sos1_removed_reasons_df_after_conversion(snapshot):
    """`Solution.constraints_df(kind="sos1", include=(..., "removed_reason"))`
    after `convert_sos1_to_constraints` carries the conversion reason and
    the comma-joined `constraint_ids` parameter."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sos1_constraints={7: Sos1Constraint(variables=[0, 1, 2])},
        sense=Instance.MINIMIZE,
    )
    instance.convert_sos1_to_constraints(7)
    sol = instance.evaluate({0: 1.0, 1: 0.0, 2: 0.0})
    assert (
        _df_snap(
            sol.constraints_df(kind="sos1", include=["metadata", "removed_reason"])
        )
        == snapshot
    )


@pytest.fixture
def _sample_set_one_hot():
    """SampleSet with two samples: 0 feasible (active=1), 1 infeasible."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=[1, 2, 3])},
        sense=Instance.MAXIMIZE,
    )
    return instance.evaluate_samples(
        {
            0: {1: 1.0, 2: 0.0, 3: 0.0},
            1: {1: 1.0, 2: 1.0, 3: 0.0},
        }
    )


def test_sample_set_one_hot_constraints_df_dynamic_columns(
    snapshot, _sample_set_one_hot
):
    """`SampleSet.constraints_df(kind="one_hot")` static columns + per-sample
    `feasible.{sample_id}` / `active_variable.{sample_id}` columns."""
    assert _df_snap(_sample_set_one_hot.constraints_df(kind="one_hot")) == snapshot


@pytest.fixture
def _sample_set_sos1():
    """SampleSet with three samples: 0 active=2, 1 all-zero (feasible, NA), 2 infeasible."""
    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sos1_constraints={20: Sos1Constraint(variables=[1, 2, 3])},
        sense=Instance.MINIMIZE,
    )
    return instance.evaluate_samples(
        {
            0: {1: 0.0, 2: 3.0, 3: 0.0},
            1: {1: 0.0, 2: 0.0, 3: 0.0},
            2: {1: 1.0, 2: 2.0, 3: 0.0},
        }
    )


def test_sample_set_sos1_constraints_df_dynamic_columns(snapshot, _sample_set_sos1):
    """`SampleSet.constraints_df(kind="sos1")` per-sample feasibility +
    active_variable columns."""
    assert _df_snap(_sample_set_sos1.constraints_df(kind="sos1")) == snapshot


def test_sample_set_one_hot_removed_reasons_df_after_conversion(snapshot):
    """`SampleSet.constraints_df(kind="one_hot", include=(..., "removed_reason"))`
    after `convert_one_hot_to_constraint` surfaces conversion reasons."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={7: OneHotConstraint(variables=[0, 1, 2])},
        sense=Instance.MINIMIZE,
    )
    instance.convert_one_hot_to_constraint(7)
    ss = instance.evaluate_samples({0: {0: 1.0, 1: 0.0, 2: 0.0}})
    assert (
        _df_snap(
            ss.constraints_df(kind="one_hot", include=["metadata", "removed_reason"])
        )
        == snapshot
    )
