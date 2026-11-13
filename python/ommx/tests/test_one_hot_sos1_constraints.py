"""Tests for OneHotConstraint and Sos1Constraint as first-class constraint types."""

from ommx import ConstraintRole
from ommx import ConstraintKind

from ommx import OneHotConstraintID, Sos1ConstraintID
from ommx import (
    RegularViolationDiagnostics,
    IndicatorViolationDiagnostics,
    Sos1ViolationDiagnostics,
)

from typing import cast, get_args

import pandas as pd
import pytest
from ommx import (
    Sense,
    AttachedDecisionVariable,
    DecisionVariable,
    Instance,
    OneHotConstraint,
    Parameter,
    Sos1Constraint,
    VariableIDLike,
)


def _df_snap(df: pd.DataFrame) -> str:
    """Deterministic, snapshot-friendly rendering of a DataFrame."""
    return df.to_string(na_rep="<NA>")


def test_variable_id_like_is_a_public_type_alias():
    assert set(get_args(VariableIDLike)) == {
        int,
        DecisionVariable,
        AttachedDecisionVariable,
    }


def test_one_hot_constraint_from_components():
    """Test creating an instance with OneHotConstraint via from_components."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    oh = OneHotConstraint(variables=x)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        active_constraints={},
        active_one_hot_constraints={10: oh},
        sense=Sense.Minimize,
    )

    assert (
        len(
            {
                cid
                for cid in instance.active_constraints
                if isinstance(cid, OneHotConstraintID)
            }
        )
        == 1
    )
    assert OneHotConstraintID(10) in (
        cid
        for cid in instance.active_constraints
        if isinstance(cid, OneHotConstraintID)
    )
    assert instance.active_constraints[OneHotConstraintID(10)].variables == [1, 2, 3]


def test_one_hot_constraint_accepts_variable_ids():
    """Structural constraints accept raw IDs when no variable metadata is needed."""
    one_hot = OneHotConstraint(variables=[1, 2, 3])

    assert one_hot.variables == [1, 2, 3]


def test_one_hot_constraint_rejects_empty_variables_with_value_error():
    with pytest.raises(ValueError, match="at least one variable"):
        OneHotConstraint(variables=[])


def test_one_hot_constraint_rejects_bool_as_variable_id():
    with pytest.raises(TypeError, match="Expected int, DecisionVariable"):
        OneHotConstraint(variables=[True])


def test_variable_id_like_rejects_other_id_bearing_objects():
    parameter = Parameter(id=1)

    with pytest.raises(TypeError, match="Expected int, DecisionVariable"):
        OneHotConstraint(variables=[cast(VariableIDLike, parameter)])


def test_sos1_constraint_from_components():
    """Test creating an instance with Sos1Constraint via from_components."""
    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    objective = sum(x)

    sos1 = Sos1Constraint(variables=x)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        active_constraints={},
        active_sos1_constraints={20: sos1},
        sense=Sense.Minimize,
    )

    assert (
        len(
            {
                cid
                for cid in instance.active_constraints
                if isinstance(cid, Sos1ConstraintID)
            }
        )
        == 1
    )
    assert Sos1ConstraintID(20) in (
        cid for cid in instance.active_constraints if isinstance(cid, Sos1ConstraintID)
    )
    assert instance.active_constraints[Sos1ConstraintID(20)].variables == [1, 2, 3]


def test_sos1_constraint_accepts_variable_ids():
    """Structural constraints accept raw IDs when no variable metadata is needed."""
    sos1 = Sos1Constraint(variables=[1, 2, 3])

    assert sos1.variables == [1, 2, 3]


def test_sos1_constraint_rejects_empty_variables_with_value_error():
    with pytest.raises(ValueError, match="at least one variable"):
        Sos1Constraint(variables=[])


def test_sos1_constraint_rejects_bool_as_variable_id():
    with pytest.raises(TypeError, match="Expected int, DecisionVariable"):
        Sos1Constraint(variables=[True])


def test_one_hot_variable_not_defined():
    """Test that OneHotConstraint with undefined variable ID fails."""
    x = [DecisionVariable.binary(1)]
    objective = x[0]

    oh = OneHotConstraint(
        variables=[x[0], DecisionVariable.binary(999)]
    )  # 999 doesn't exist

    with pytest.raises(RuntimeError):
        Instance.from_components(
            decision_variables=x,
            objective=objective,
            active_constraints={},
            active_one_hot_constraints={10: oh},
            sense=Sense.Minimize,
        )


def test_sos1_variable_not_defined():
    """Test that Sos1Constraint with undefined variable ID fails."""
    x = [DecisionVariable.continuous(1, lower=0, upper=10)]
    objective = x[0]

    sos1 = Sos1Constraint(
        variables=[x[0], DecisionVariable.continuous(999, lower=0, upper=10)]
    )  # 999 doesn't exist

    with pytest.raises(RuntimeError):
        Instance.from_components(
            decision_variables=x,
            objective=objective,
            active_constraints={},
            active_sos1_constraints={20: sos1},
            sense=Sense.Minimize,
        )


def test_one_hot_variable_not_binary():
    """The enclosing instance, not the input object, owns variable kind."""
    x = [
        DecisionVariable.binary(1),
        DecisionVariable.continuous(2, lower=0, upper=1),  # not binary
    ]
    objective = sum(x)

    oh = OneHotConstraint(
        variables=[DecisionVariable.binary(1), DecisionVariable.binary(2)]
    )

    with pytest.raises(RuntimeError, match="One-hot variable.*must be binary"):
        Instance.from_components(
            decision_variables=x,
            objective=objective,
            active_constraints={},
            active_one_hot_constraints={10: oh},
            sense=Sense.Minimize,
        )


def test_one_hot_uses_enclosing_instance_variable_kind():
    """Input object metadata is ignored when the enclosing variable is binary."""
    x = DecisionVariable.binary(1)
    reference = DecisionVariable.continuous(1, lower=0, upper=1)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        active_constraints={},
        active_one_hot_constraints={10: OneHotConstraint(variables=[reference])},
        sense=Sense.Minimize,
    )

    assert instance.active_constraints[OneHotConstraintID(10)].variables == [1]


def test_serialize_not_yet_supported():
    """Serialization of OneHot/SOS1 constraints to v1 proto is not yet supported."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        active_constraints={},
        active_one_hot_constraints={10: OneHotConstraint(variables=x)},
        sense=Sense.Minimize,
    )

    with pytest.raises(Exception, match="to_v2_bytes"):
        instance.to_v1_bytes()


def test_both_one_hot_and_sos1():
    """Test instance with both OneHot and SOS1 constraints."""
    x = [DecisionVariable.binary(i) for i in range(1, 6)]
    objective = sum(x)

    oh = OneHotConstraint(variables=x[:3])
    sos1 = Sos1Constraint(variables=x[2:])

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        active_constraints={},
        active_one_hot_constraints={10: oh},
        active_sos1_constraints={20: sos1},
        sense=Sense.Minimize,
    )

    assert (
        len(
            {
                cid
                for cid in instance.active_constraints
                if isinstance(cid, OneHotConstraintID)
            }
        )
        == 1
    )
    assert (
        len(
            {
                cid
                for cid in instance.active_constraints
                if isinstance(cid, Sos1ConstraintID)
            }
        )
        == 1
    )


def test_evaluate_with_one_hot_feasible():
    """Test that evaluation with OneHot constraints checks feasibility."""
    from ommx import State

    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    oh = OneHotConstraint(variables=x)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        active_constraints={},
        active_one_hot_constraints={10: oh},
        sense=Sense.Minimize,
    )

    # x1=0, x2=1, x3=0 → feasible (exactly one is 1)
    state = State({1: 0.0, 2: 1.0, 3: 0.0})
    solution = instance.evaluate(state)
    assert solution.feasible


def test_evaluate_with_one_hot_infeasible():
    """Test that evaluation with OneHot constraints detects infeasibility."""
    from ommx import State

    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    oh = OneHotConstraint(variables=x)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        active_constraints={},
        active_one_hot_constraints={10: oh},
        sense=Sense.Minimize,
    )

    # x1=1, x2=1, x3=0 → infeasible (two are 1)
    state = State({1: 1.0, 2: 1.0, 3: 0.0})
    solution = instance.evaluate(state)
    assert not solution.feasible


def test_evaluate_with_sos1_feasible():
    """Test that evaluation with SOS1 constraints checks feasibility."""
    from ommx import State

    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    objective = sum(x)

    sos1 = Sos1Constraint(variables=x)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        active_constraints={},
        active_sos1_constraints={20: sos1},
        sense=Sense.Minimize,
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
        active_constraints={},
        active_sos1_constraints={10: Sos1Constraint(variables=x)},
        sense=Sense.Minimize,
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
    # Original SOS1 is retained on the inactive side with our reason string.
    inactive = instance.inactive_constraints[Sos1ConstraintID(10)]
    assert inactive.inactive_reason == "ommx.Instance.convert_sos1_to_constraints"
    # `constraint_ids` parameter lists all 5 new IDs in insertion order.
    assert inactive.inactive_reason_parameters["constraint_ids"] == "11,12,13,14,15"


def test_convert_sos1_rejects_domain_excluding_zero():
    """SOS1 over x with bound [1, 3] cannot be Big-M converted: y=0 ⇒ x=0 is infeasible."""
    x = [DecisionVariable.integer(1, lower=1, upper=3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        active_constraints={},
        active_sos1_constraints={10: Sos1Constraint(variables=x)},
        sense=Sense.Minimize,
    )
    before_var_ids = {dv.id for dv in instance.decision_variables}

    with pytest.raises(RuntimeError, match="excludes 0"):
        instance.convert_sos1_to_constraints(10)

    # Instance unchanged on error.
    assert {dv.id for dv in instance.decision_variables} == before_var_ids
    assert Sos1ConstraintID(10) in (
        cid for cid in instance.active_constraints if isinstance(cid, Sos1ConstraintID)
    )


def test_sos1_constraints_df_roundtrips_inactive_context(snapshot):
    """`constraints_df(kind=ConstraintKind.Sos1, min_role=ConstraintRole.EVALUATION_ONLY)` surfaces reason + constraint_ids
    parameter as columns on inactive rows. Active is empty after conversion."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={},
        active_sos1_constraints={7: Sos1Constraint(variables=x)},
        sense=Sense.Minimize,
    )
    instance.convert_sos1_to_constraints(7)

    assert instance.constraints_df(
        kind=ConstraintKind.Sos1, min_role=ConstraintRole.ACTIVE
    ).empty
    assert (
        _df_snap(
            instance.constraints_df(
                kind=ConstraintKind.Sos1,
                min_role=ConstraintRole.EVALUATION_ONLY,
                include=["label", "parameters", "inactive_reason"],
            )
        )
        == snapshot
    )


def test_evaluate_with_sos1_infeasible():
    """Test that evaluation with SOS1 constraints detects infeasibility."""
    from ommx import State

    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    objective = sum(x)

    sos1 = Sos1Constraint(variables=x)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        active_constraints={},
        active_sos1_constraints={20: sos1},
        sense=Sense.Minimize,
    )

    # x1=1, x2=2, x3=0 → infeasible (two non-zero)
    state = State({1: 1.0, 2: 2.0, 3: 0.0})
    solution = instance.evaluate(state)
    assert not solution.feasible


@pytest.fixture
def _solution_one_hot_feasible():
    """One-hot constraint with a feasible solution (selected_variable=2)."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={},
        active_one_hot_constraints={10: OneHotConstraint(variables=x)},
        sense=Sense.Maximize,
    )
    return instance.evaluate({1: 0.0, 2: 1.0, 3: 0.0})


@pytest.fixture
def _solution_one_hot_infeasible():
    """One-hot constraint with an infeasible solution (selected_variable=NA)."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={},
        active_one_hot_constraints={10: OneHotConstraint(variables=x)},
        sense=Sense.Maximize,
    )
    return instance.evaluate({1: 1.0, 2: 1.0, 3: 0.0})


def test_solution_one_hot_constraints_df_feasible(snapshot, _solution_one_hot_feasible):
    """`Solution.constraints_df(kind=ConstraintKind.OneHot)` for feasible: feasible=True,
    selected_variable=2."""
    assert (
        _solution_one_hot_feasible.constraints[OneHotConstraintID(10)].selected_variable
        == 2
    )
    assert (
        _df_snap(_solution_one_hot_feasible.constraints_df(kind=ConstraintKind.OneHot))
        == snapshot
    )


def test_solution_one_hot_constraints_df_infeasible(
    snapshot, _solution_one_hot_infeasible
):
    """`Solution.constraints_df(kind=ConstraintKind.OneHot)` for infeasible: feasible=False,
    selected_variable=NA."""
    assert (
        _solution_one_hot_infeasible.constraints[
            OneHotConstraintID(10)
        ].selected_variable
        is None
    )
    assert (
        _df_snap(
            _solution_one_hot_infeasible.constraints_df(kind=ConstraintKind.OneHot)
        )
        == snapshot
    )


@pytest.fixture
def _solution_sos1_one_nonzero():
    """SOS1 with exactly one nonzero variable → feasible, selected_variable=2."""
    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={},
        active_sos1_constraints={20: Sos1Constraint(variables=x)},
        sense=Sense.Minimize,
    )
    return instance.evaluate({1: 0.0, 2: 5.0, 3: 0.0})


@pytest.fixture
def _solution_sos1_all_zero():
    """SOS1 with all zero → feasible (≤1 nonzero), selected_variable=NA."""
    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={},
        active_sos1_constraints={20: Sos1Constraint(variables=x)},
        sense=Sense.Minimize,
    )
    return instance.evaluate({1: 0.0, 2: 0.0, 3: 0.0})


def test_solution_sos1_constraints_df_one_nonzero(snapshot, _solution_sos1_one_nonzero):
    """`Solution.constraints_df(kind=ConstraintKind.Sos1)` with one nonzero: feasible=True,
    selected_variable=2."""
    assert (
        _solution_sos1_one_nonzero.constraints[Sos1ConstraintID(20)].selected_variable
        == 2
    )
    assert (
        _df_snap(_solution_sos1_one_nonzero.constraints_df(kind=ConstraintKind.Sos1))
        == snapshot
    )


def test_solution_sos1_constraints_df_all_zero(snapshot, _solution_sos1_all_zero):
    """`Solution.constraints_df(kind=ConstraintKind.Sos1)` with all-zero: feasible=True,
    selected_variable=NA."""
    assert (
        _solution_sos1_all_zero.constraints[Sos1ConstraintID(20)].selected_variable
        is None
    )
    assert (
        _df_snap(_solution_sos1_all_zero.constraints_df(kind=ConstraintKind.Sos1))
        == snapshot
    )


def test_solution_one_hot_inactive_reasons_df_after_conversion(snapshot):
    """`Solution.constraints_df(kind=ConstraintKind.OneHot, include=(..., "inactive_reason"))`
    after `convert_one_hot_to_constraint` carries the conversion reason."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={},
        active_one_hot_constraints={7: OneHotConstraint(variables=x)},
        sense=Sense.Minimize,
    )
    instance.convert_one_hot_to_constraint(7)
    sol = instance.evaluate({0: 1.0, 1: 0.0, 2: 0.0})
    assert (
        _df_snap(
            sol.constraints_df(
                kind=ConstraintKind.OneHot, include=["label", "inactive_reason"]
            )
        )
        == snapshot
    )


def test_solution_sos1_inactive_reasons_df_after_conversion(snapshot):
    """`Solution.constraints_df(kind=ConstraintKind.Sos1, include=(..., "inactive_reason"))`
    after `convert_sos1_to_constraints` carries the conversion reason and
    the comma-joined `constraint_ids` parameter."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={},
        active_sos1_constraints={7: Sos1Constraint(variables=x)},
        sense=Sense.Minimize,
    )
    instance.convert_sos1_to_constraints(7)
    sol = instance.evaluate({0: 1.0, 1: 0.0, 2: 0.0})
    assert (
        _df_snap(
            sol.constraints_df(
                kind=ConstraintKind.Sos1, include=["label", "inactive_reason"]
            )
        )
        == snapshot
    )


@pytest.fixture
def _sample_set_one_hot():
    """SampleSet with two samples: 0 feasible (selected=1), 1 infeasible."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={},
        active_one_hot_constraints={10: OneHotConstraint(variables=x)},
        sense=Sense.Maximize,
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
    """`SampleSet.constraints_df(kind=ConstraintKind.OneHot)` static columns + per-sample
    `feasible.{sample_id}` / `selected_variable.{sample_id}` columns."""
    assert _sample_set_one_hot.constraints[
        OneHotConstraintID(10)
    ].selected_variable == {
        0: 1,
        1: None,
    }
    assert (
        _df_snap(_sample_set_one_hot.constraints_df(kind=ConstraintKind.OneHot))
        == snapshot
    )


@pytest.fixture
def _sample_set_sos1():
    """SampleSet with three samples: 0 selected=2, 1 all-zero (feasible, NA), 2 infeasible."""
    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={},
        active_sos1_constraints={20: Sos1Constraint(variables=x)},
        sense=Sense.Minimize,
    )
    return instance.evaluate_samples(
        {
            0: {1: 0.0, 2: 3.0, 3: 0.0},
            1: {1: 0.0, 2: 0.0, 3: 0.0},
            2: {1: 1.0, 2: 2.0, 3: 0.0},
        }
    )


def test_sample_set_sos1_constraints_df_dynamic_columns(snapshot, _sample_set_sos1):
    """`SampleSet.constraints_df(kind=ConstraintKind.Sos1)` per-sample feasibility +
    selected_variable columns."""
    assert _sample_set_sos1.constraints[Sos1ConstraintID(20)].selected_variable == {
        0: 2,
        1: None,
        2: None,
    }
    assert (
        _df_snap(_sample_set_sos1.constraints_df(kind=ConstraintKind.Sos1)) == snapshot
    )


def test_sample_set_one_hot_inactive_reasons_df_after_conversion(snapshot):
    """`SampleSet.constraints_df(kind=ConstraintKind.OneHot, include=(..., "inactive_reason"))`
    after `convert_one_hot_to_constraint` surfaces conversion reasons."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        active_constraints={},
        active_one_hot_constraints={7: OneHotConstraint(variables=x)},
        sense=Sense.Minimize,
    )
    instance.convert_one_hot_to_constraint(7)
    ss = instance.evaluate_samples({0: {0: 1.0, 1: 0.0, 2: 0.0}})
    assert (
        _df_snap(
            ss.constraints_df(
                kind=ConstraintKind.OneHot, include=["label", "inactive_reason"]
            )
        )
        == snapshot
    )


def test_sos1_small_member_errors_are_feasible_without_changing_values():
    variables = [DecisionVariable.continuous(i, lower=-10, upper=10) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=variables,
        objective=0,
        active_constraints={},
        active_sos1_constraints={7: Sos1Constraint(variables=variables)},
        sense=Sense.Minimize,
    )
    state = {0: 1.0, 1: 0.00006, 2: -0.00006}
    solution = instance.evaluate(state, atol=0.0001)
    assert solution.feasible
    row = solution.constraints[Sos1ConstraintID(7)]
    assert solution.state.entries == state
    assert Sos1ConstraintID(7) not in solution.violation_diagnostics()
    assert row.violation() == 0.00006


@pytest.mark.parametrize("inactive", [False, True])
def test_one_hot_violation_details_survive_storage_and_sample_extraction(inactive):
    from ommx import OneHotViolationDiagnostics, SampleSet, Solution

    variables = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=variables,
        objective=0,
        active_constraints={},
        active_one_hot_constraints={7: OneHotConstraint(variables=variables)},
        sense=Sense.Minimize,
    )
    if inactive:
        instance.convert_one_hot_to_constraint(7)
    states = {
        2: {0: 0.0, 1: 0.0, 2: 0.0},
        5: {0: 1.0, 1: 1.0, 2: -0.5},
        8: {0: 0.9, 1: 0.2, 2: 0.0},
        11: {0: 0.0, 1: 1.0, 2: 0.0},
        14: {0: 0.5, 1: 0.5, 2: 0.0},
    }
    constraint_id = OneHotConstraintID(7)
    atol = 1e-6
    samples = instance.evaluate_samples(states, atol=atol)
    for result in [samples, SampleSet.from_v2_bytes(samples.to_v2_bytes())]:
        details = {
            sid: result.violation_diagnostics(sid).get(constraint_id) for sid in states
        }
        assert set(details) == set(states)
        for sample_id, state in states.items():
            solution = instance.evaluate(state, atol=atol)
            for candidate in [
                solution,
                Solution.from_v2_bytes(solution.to_v2_bytes()),
                result.get(sample_id),
            ]:
                row = candidate.constraints[constraint_id]
                diagnostic = candidate.violation_diagnostics().get(constraint_id)
                assert diagnostic == details[sample_id]
                assert (
                    constraint_id not in candidate.violation_diagnostics()
                ) == row.is_feasible(atol=atol)
                assert (diagnostic is None) == row.is_feasible(atol=atol)
                if sample_id == 11:
                    assert diagnostic is None
                    continue
                assert isinstance(
                    diagnostic,
                    (
                        OneHotViolationDiagnostics.MissingSelection,
                        OneHotViolationDiagnostics.MultipleSelections,
                        OneHotViolationDiagnostics.ValueErrors,
                    ),
                )
                assert diagnostic.values == state
                if sample_id == 2:
                    assert isinstance(
                        diagnostic, OneHotViolationDiagnostics.MissingSelection
                    )
                    assert row.violation() == 1.0
                elif sample_id == 5:
                    assert isinstance(
                        diagnostic, OneHotViolationDiagnostics.MultipleSelections
                    )
                    assert diagnostic.selected_variables == {0, 1}
                    assert row.violation() == 1.0
                else:
                    assert isinstance(
                        diagnostic, OneHotViolationDiagnostics.ValueErrors
                    )
                    if sample_id == 8:
                        assert diagnostic.target_variable == 0
                        assert diagnostic.errors == pytest.approx({0: 0.1, 1: 0.2})
                    else:
                        assert diagnostic.target_variable == 1
                        assert diagnostic.errors == {0: 0.5, 1: 0.5}
                    assert max(diagnostic.errors.values()) == row.violation()
                # The returned maps are snapshots, not mutable result storage.
                diagnostic.values.clear()
                fresh = candidate.violation_diagnostics()[constraint_id]
                assert isinstance(
                    fresh,
                    (
                        OneHotViolationDiagnostics.MissingSelection,
                        OneHotViolationDiagnostics.MultipleSelections,
                        OneHotViolationDiagnostics.ValueErrors,
                    ),
                )
                assert fresh.values == state


@pytest.mark.parametrize("atol", [0.125, 0.5, 1.0, 2.0])
def test_one_hot_diagnostics_follow_retained_tolerance(atol):
    variables = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=variables,
        objective=0,
        active_constraints={},
        active_one_hot_constraints={7: OneHotConstraint(variables=variables)},
        sense=Sense.Minimize,
    )
    for state in [{0: 0, 1: 0, 2: 0}, {0: 1, 1: 1, 2: 0}, {0: 0.875, 1: 0.25, 2: 0}]:
        solution = instance.evaluate(state, atol=atol)
        row = solution.constraints[OneHotConstraintID(7)]
        before = solution.violation_diagnostics().get(OneHotConstraintID(7))
        assert (
            OneHotConstraintID(7) not in solution.violation_diagnostics()
        ) == row.is_feasible(atol=atol)
        row.is_feasible(atol=10.0)
        assert solution.violation_diagnostics().get(OneHotConstraintID(7)) == before


@pytest.mark.parametrize("inactive", [False, True])
def test_sos1_violating_values_match_feasibility_and_survive_storage(inactive):
    from ommx import SampleSet, Solution

    variables = [DecisionVariable.continuous(i, lower=-10, upper=10) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=variables,
        objective=0,
        active_constraints={},
        active_sos1_constraints={7: Sos1Constraint(variables=variables)},
        sense=Sense.Minimize,
    )
    if inactive:
        instance.convert_sos1_to_constraints(7)
    states = {
        2: {0: 0.0, 1: 0.0, 2: 0.0},
        5: {0: 2.0, 1: -3.0, 2: 0.0},
        8: {0: 1.0, 1: 0.125, 2: -0.125},
        11: {0: 1.0, 1: -1.0, 2: 0.0},
    }
    if inactive:
        for state in states.values():
            retained = max(range(3), key=lambda i: (abs(state[i]), i))
            state.update({3 + i: float(i == retained) for i in range(3)})
    expected = {2: {}, 5: {0: 2.0}, 8: {}, 11: {0: 1.0}}
    constraint_id = Sos1ConstraintID(7)
    samples = instance.evaluate_samples(states, atol=0.125)
    for result in [samples, SampleSet.from_v2_bytes(samples.to_v2_bytes())]:
        for sample_id, state in states.items():
            sampled_diagnostic = result.violation_diagnostics(sample_id).get(
                constraint_id
            )
            if expected[sample_id]:
                assert isinstance(sampled_diagnostic, Sos1ViolationDiagnostics)
                assert sampled_diagnostic.values == expected[sample_id]
            else:
                assert sampled_diagnostic is None
            solution = instance.evaluate(state, atol=0.125)
            for candidate in [
                solution,
                Solution.from_v2_bytes(solution.to_v2_bytes()),
                result.get(sample_id),
            ]:
                row = candidate.constraints[constraint_id]
                diagnostic = candidate.violation_diagnostics().get(constraint_id)
                assert diagnostic == sampled_diagnostic
                if expected[sample_id]:
                    assert isinstance(diagnostic, Sos1ViolationDiagnostics)
                    assert diagnostic.values == expected[sample_id]
                    diagnostic.values.clear()
                    assert diagnostic.values == expected[sample_id]
                    with pytest.raises(AttributeError):
                        setattr(diagnostic, "values", {})
                else:
                    assert diagnostic is None
                assert (
                    constraint_id not in candidate.violation_diagnostics()
                ) == row.is_feasible(atol=0.125)
                row.is_feasible(atol=10.0)
                assert (
                    candidate.violation_diagnostics().get(constraint_id) == diagnostic
                )


def test_violation_diagnostics_all_families_and_unknown_samples():
    from ommx import (
        ConstraintID,
        IndicatorConstraintID,
        OneHotViolationDiagnostics,
        SampleSet,
        Solution,
    )

    x, y = DecisionVariable.binary(0), DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=0,
        sense=Sense.Minimize,
        active_constraints={0: x == 0, 1: y == 1},
        active_indicator_constraints={2: (y == 0).with_indicator(x)},
        active_one_hot_constraints={3: OneHotConstraint(variables=[x, y])},
        active_sos1_constraints={4: Sos1Constraint(variables=[x, y])},
    )
    states = {7: {0: 1, 1: 1}, 8: {0: 0, 1: 1}, 9: {0: 0, 1: 0}}
    expected_ids = {
        7: {
            ConstraintID(0),
            IndicatorConstraintID(2),
            OneHotConstraintID(3),
            Sos1ConstraintID(4),
        },
        8: set(),
        9: {ConstraintID(1), OneHotConstraintID(3)},
    }
    samples = instance.evaluate_samples(states)
    for result in [samples, SampleSet.from_v2_bytes(samples.to_v2_bytes())]:
        for sid, state in states.items():
            diagnostics = result.violation_diagnostics(sid)
            assert set(diagnostics) == expected_ids[sid]
            direct = instance.evaluate(state)
            for candidate in [
                direct,
                result.get(sid),
                Solution.from_v2_bytes(direct.to_v2_bytes()),
            ]:
                assert candidate.violation_diagnostics() == diagnostics
                assert set(diagnostics) == {
                    cid
                    for cid in candidate.constraints
                    if candidate.constraint_violation(cid) > candidate.feasibility_atol
                }
            if sid == 7:
                assert isinstance(
                    diagnostics[ConstraintID(0)], RegularViolationDiagnostics
                )
                assert isinstance(
                    diagnostics[IndicatorConstraintID(2)], IndicatorViolationDiagnostics
                )
                assert isinstance(
                    diagnostics[OneHotConstraintID(3)],
                    OneHotViolationDiagnostics.MultipleSelections,
                )
                sos1_diagnostic = diagnostics[Sos1ConstraintID(4)]
                assert isinstance(sos1_diagnostic, Sos1ViolationDiagnostics)
                assert sos1_diagnostic.values == {0: 1}
            diagnostics.clear()
            assert set(result.violation_diagnostics(sid)) == expected_ids[sid]
        with pytest.raises(KeyError, match="Unknown sample ID"):
            result.violation_diagnostics(999)
    # Missing samples must not silently return {} even with no constraint rows.
    empty = Instance.minimize().evaluate_samples({7: {}})
    assert empty.violation_diagnostics(7) == {}
    with pytest.raises(KeyError, match="Unknown sample ID"):
        empty.violation_diagnostics(999)


def test_lazy_diagnostics_include_evaluation_only_constraints():
    from ommx import ConstraintRole, OneHotViolationDiagnostics, SampleSet, Solution

    instance = Instance.minimize()
    x = [instance.new_binary() for _ in range(2)]
    one_hot = instance.add_one_hot_constraint(
        OneHotConstraint(variables=x)
    ).constraint_id
    sos1 = instance.add_sos1_constraint(Sos1Constraint(variables=x)).constraint_id
    instance.deactivate_one_hot_constraint(
        one_hot, "diagnostic", skip_feasibility_check=True
    )
    instance.deactivate_sos1_constraint(sos1, "diagnostic", skip_feasibility_check=True)
    state = {v.id: 1 for v in x}
    samples = instance.evaluate_samples({7: state})
    for result in [samples, SampleSet.from_v2_bytes(samples.to_v2_bytes())]:
        assert result.feasible == {7: True}
        assert (
            result.violation_diagnostics(7, min_role=ConstraintRole.FEASIBILITY_CHECK)
            == {}
        )
        assert result.violation_diagnostics(7, min_role=ConstraintRole.ACTIVE) == {}
        assert isinstance(
            result.violation_diagnostics(7)[one_hot],
            OneHotViolationDiagnostics.MultipleSelections,
        )
        sos1_diagnostic = result.violation_diagnostics(7)[sos1]
        assert isinstance(sos1_diagnostic, Sos1ViolationDiagnostics)
        assert sos1_diagnostic.values == {x[0].id: 1}
        solution = result.get(7)
        for candidate in [solution, Solution.from_v2_bytes(solution.to_v2_bytes())]:
            assert candidate.feasible
            assert (
                candidate.violation_diagnostics(
                    min_role=ConstraintRole.FEASIBILITY_CHECK
                )
                == {}
            )
            assert isinstance(
                candidate.violation_diagnostics()[one_hot],
                OneHotViolationDiagnostics.MultipleSelections,
            )
            sos1_diagnostic = candidate.violation_diagnostics()[sos1]
            assert isinstance(sos1_diagnostic, Sos1ViolationDiagnostics)
            assert sos1_diagnostic.values == {x[0].id: 1}


def test_empty_violation_diagnostics_do_not_imply_variable_domain_feasibility():
    instance = Instance.minimize()
    x = instance.new_binary()
    instance.objective = x
    solution = instance.evaluate({x.id: 0.5})
    assert not solution.feasible
    assert solution.violation_diagnostics() == {}
    samples = instance.evaluate_samples({7: {x.id: 0.5}})
    assert samples.feasible == {7: False}
    assert samples.violation_diagnostics(7) == {}
