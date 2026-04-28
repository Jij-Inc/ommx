"""Tests for OneHotConstraint and Sos1Constraint as first-class constraint types."""

import pytest
from ommx.v1 import Instance, DecisionVariable, OneHotConstraint, Sos1Constraint


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


def test_sos1_constraints_df_roundtrips_removed_metadata():
    """removed_sos1_constraints_df surfaces reason + constraint_ids parameter as columns."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sos1_constraints={7: Sos1Constraint(variables=[0, 1, 2])},
        sense=Instance.MINIMIZE,
    )
    new_ids = instance.convert_sos1_to_constraints(7)

    active_df = instance.sos1_constraints_df()
    assert active_df.empty

    removed_df = instance.removed_sos1_constraints_df()
    assert list(removed_df.index) == [7]
    assert (
        removed_df.loc[7, "removed_reason"]
        == "ommx.Instance.convert_sos1_to_constraints"
    )
    assert removed_df.loc[7, "removed_reason.constraint_ids"] == ",".join(
        str(i) for i in new_ids
    )


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


def test_solution_one_hot_constraints_df_surfaces_active_variable():
    """`Solution.one_hot_constraints_df` exposes feasibility and the active variable
    ID for each one-hot constraint. Feasible case: active_variable is set. Infeasible:
    active_variable is NA."""
    import pandas as pd

    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=[1, 2, 3])},
        sense=Instance.MAXIMIZE,
    )

    sol_feas = instance.evaluate({1: 0.0, 2: 1.0, 3: 0.0})
    df_feas = sol_feas.one_hot_constraints_df()
    assert list(df_feas.index) == [10]
    assert df_feas.loc[10, "feasible"]
    assert df_feas.loc[10, "active_variable"] == 2
    assert df_feas.loc[10, "used_ids"] == {1, 2, 3}

    sol_infeas = instance.evaluate({1: 1.0, 2: 1.0, 3: 0.0})
    df_infeas = sol_infeas.one_hot_constraints_df()
    assert not df_infeas.loc[10, "feasible"]
    assert pd.isna(df_infeas.loc[10, "active_variable"])


def test_solution_sos1_constraints_df_surfaces_active_variable():
    """`Solution.sos1_constraints_df` exposes feasibility and the active variable
    ID. All-zero is feasible for SOS1 and active_variable is NA."""
    import pandas as pd

    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sos1_constraints={20: Sos1Constraint(variables=[1, 2, 3])},
        sense=Instance.MINIMIZE,
    )

    sol_one_active = instance.evaluate({1: 0.0, 2: 5.0, 3: 0.0})
    df = sol_one_active.sos1_constraints_df()
    assert list(df.index) == [20]
    assert df.loc[20, "feasible"]
    assert df.loc[20, "active_variable"] == 2

    sol_all_zero = instance.evaluate({1: 0.0, 2: 0.0, 3: 0.0})
    df_zero = sol_all_zero.sos1_constraints_df()
    assert df_zero.loc[20, "feasible"]
    assert pd.isna(df_zero.loc[20, "active_variable"])


def test_solution_one_hot_removed_reasons_df_after_conversion():
    """After `convert_one_hot_to_constraint`, the one-hot appears in
    `Solution.one_hot_removed_reasons_df` with the conversion reason."""
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

    active_df = sol.one_hot_constraints_df()
    assert list(active_df.index) == [7]

    removed_df = sol.one_hot_removed_reasons_df()
    assert list(removed_df.index) == [7]
    assert (
        removed_df.loc[7, "removed_reason"]
        == "ommx.Instance.convert_one_hot_to_constraint"
    )


def test_solution_sos1_removed_reasons_df_after_conversion():
    """After `convert_sos1_to_constraints`, the SOS1 constraint appears in
    `Solution.sos1_removed_reasons_df` with the conversion reason."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sos1_constraints={7: Sos1Constraint(variables=[0, 1, 2])},
        sense=Instance.MINIMIZE,
    )
    new_ids = instance.convert_sos1_to_constraints(7)

    sol = instance.evaluate({0: 1.0, 1: 0.0, 2: 0.0})

    removed_df = sol.sos1_removed_reasons_df()
    assert list(removed_df.index) == [7]
    assert (
        removed_df.loc[7, "removed_reason"]
        == "ommx.Instance.convert_sos1_to_constraints"
    )
    assert removed_df.loc[7, "removed_reason.constraint_ids"] == ",".join(
        str(i) for i in new_ids
    )


def test_sample_set_one_hot_constraints_df_dynamic_columns():
    """`SampleSet.one_hot_constraints_df` has static metadata columns plus per-sample
    `feasible.{sample_id}` and `active_variable.{sample_id}` columns."""
    import pandas as pd

    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={10: OneHotConstraint(variables=[1, 2, 3])},
        sense=Instance.MAXIMIZE,
    )
    ss = instance.evaluate_samples(
        {
            0: {1: 1.0, 2: 0.0, 3: 0.0},  # feasible, active=1
            1: {1: 1.0, 2: 1.0, 3: 0.0},  # infeasible
        }
    )

    df = ss.one_hot_constraints_df()
    assert list(df.index) == [10]
    assert df.loc[10, "used_ids"] == {1, 2, 3}
    assert df.loc[10, "feasible.0"]
    assert df.loc[10, "active_variable.0"] == 1
    assert not df.loc[10, "feasible.1"]
    assert pd.isna(df.loc[10, "active_variable.1"])


def test_sample_set_sos1_constraints_df_dynamic_columns():
    """`SampleSet.sos1_constraints_df` mirrors the one-hot shape with per-sample
    feasibility and active_variable columns."""
    import pandas as pd

    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sos1_constraints={20: Sos1Constraint(variables=[1, 2, 3])},
        sense=Instance.MINIMIZE,
    )
    ss = instance.evaluate_samples(
        {
            0: {1: 0.0, 2: 3.0, 3: 0.0},  # feasible, active=2
            1: {1: 0.0, 2: 0.0, 3: 0.0},  # feasible, no active (NA)
            2: {1: 1.0, 2: 2.0, 3: 0.0},  # infeasible
        }
    )

    df = ss.sos1_constraints_df()
    assert list(df.index) == [20]
    assert df.loc[20, "feasible.0"]
    assert df.loc[20, "active_variable.0"] == 2
    assert df.loc[20, "feasible.1"]
    assert pd.isna(df.loc[20, "active_variable.1"])
    assert not df.loc[20, "feasible.2"]


def test_sample_set_one_hot_removed_reasons_df_after_conversion():
    """`SampleSet.one_hot_removed_reasons_df` surfaces conversion reasons the same
    way the Solution variant does."""
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

    removed_df = ss.one_hot_removed_reasons_df()
    assert list(removed_df.index) == [7]
    assert (
        removed_df.loc[7, "removed_reason"]
        == "ommx.Instance.convert_one_hot_to_constraint"
    )
