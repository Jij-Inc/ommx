import sys

import highspy
import pytest

from ommx import Constraint, DecisionVariable, Equality, Instance, Sense
from ommx.adapter import InfeasibleDetected, UnboundedDetected
from ommx_highs_adapter import OMMXHighsAdapter


@pytest.mark.parametrize("variable_mode", ["none", "unused", "active"])
@pytest.mark.parametrize("magnitude", [1, 1e20, sys.float_info.max])
@pytest.mark.parametrize(
    "equality,constant,feasible",
    [
        (Equality.EqualToZero, -1, False),
        (Equality.EqualToZero, 0, True),
        (Equality.EqualToZero, 1, False),
        (Equality.LessThanOrEqualToZero, -1, True),
        (Equality.LessThanOrEqualToZero, 0, True),
        (Equality.LessThanOrEqualToZero, 1, False),
    ],
)
def test_constant_constraint_solver_lifecycle(
    variable_mode, magnitude, equality, constant, feasible
):
    constant *= magnitude
    x = DecisionVariable.continuous(11, lower=0, upper=1)
    instance = Instance.from_components(
        decision_variables=[] if variable_mode == "none" else [x],
        objective=x + 7 if variable_mode == "active" else 7,
        constraints={42: Constraint(function=constant, equality=equality)},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()
    assert OMMXHighsAdapter.check_applicability(instance).is_member
    adapter = OMMXHighsAdapter(instance)
    model = adapter.solver_input
    assert model.getOptionValue("infinite_bound") == highspy.Highs().getOptionValue(
        "infinite_bound"
    )
    assert model.getModelStatus() == highspy.HighsModelStatus.kNotset
    assert model.getNumRow() == 1
    lp = model.getLp()
    assert lp.row_upper_[0] == -constant
    assert lp.row_lower_[0] == (
        -constant if equality == Equality.EqualToZero else -highspy.kHighsInf
    )

    assert model.run() == highspy.HighsStatus.kOk
    if feasible:
        assert model.getModelStatus() == highspy.HighsModelStatus.kOptimal
        state = adapter.decode_to_state(model)
        assert state.entries == ({11: 0.0} if variable_mode == "active" else {})
        solution = adapter.decode(model)
        assert solution.feasible
        assert solution.objective == 7
    else:
        assert model.getModelStatus() == highspy.HighsModelStatus.kInfeasible
        with pytest.raises(InfeasibleDetected):
            adapter.decode(model)

    for solve in (OMMXHighsAdapter.solve, OMMXHighsAdapter.solve_without_preparation):
        if feasible:
            solution = solve(instance)
            assert solution.feasible
            assert solution.objective == 7
        else:
            with pytest.raises(InfeasibleDetected):
                solve(instance)
    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize(
    "equality", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
@pytest.mark.parametrize("constant", [-1, 0, 1])
def test_constant_constraint_with_unbounded_objective(equality, constant):
    x = DecisionVariable.continuous(11)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=-x,
        constraints={42: Constraint(function=constant, equality=equality)},
        sense=Sense.Minimize,
    )
    feasible = constant == 0 if equality == Equality.EqualToZero else constant <= 0
    expected_exception = UnboundedDetected if feasible else InfeasibleDetected
    adapter = OMMXHighsAdapter(instance)
    model = adapter.solver_input
    model.run()
    assert model.getModelStatus() == (
        highspy.HighsModelStatus.kUnbounded
        if feasible
        else highspy.HighsModelStatus.kInfeasible
    )
    with pytest.raises(expected_exception):
        adapter.decode(model)
    with pytest.raises(expected_exception):
        OMMXHighsAdapter.solve(instance)


@pytest.mark.parametrize(
    "equality", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
@pytest.mark.parametrize("tolerance,feasible", [(1e-7, False), (1e-4, True)])
def test_constant_constraint_uses_solver_tolerance(equality, tolerance, feasible):
    instance = Instance.from_components(
        decision_variables=[],
        objective=7,
        constraints={42: Constraint(function=1e-5, equality=equality)},
        sense=Sense.Minimize,
    )
    assert OMMXHighsAdapter.check_applicability(instance).is_member
    model = OMMXHighsAdapter(instance).solver_input
    model.setOptionValue("primal_feasibility_tolerance", tolerance)
    model.run()
    assert model.getModelStatus() == (
        highspy.HighsModelStatus.kOptimal
        if feasible
        else highspy.HighsModelStatus.kInfeasible
    )


def test_large_constant_override_does_not_affect_other_rows():
    x = DecisionVariable.continuous(11, lower=0, upper=1)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={
            1: x <= 1e30,
            2: Constraint(
                function=-sys.float_info.max,
                equality=Equality.LessThanOrEqualToZero,
            ),
            3: x <= 1e30,
        },
        sense=Sense.Minimize,
    )
    model = OMMXHighsAdapter(instance).solver_input
    # The neighboring nonconstant rows retain HiGHS's default bound handling.
    assert list(model.getLp().row_upper_) == [
        highspy.kHighsInf,
        sys.float_info.max,
        highspy.kHighsInf,
    ]
    assert model.getOptionValue("infinite_bound") == highspy.Highs().getOptionValue(
        "infinite_bound"
    )


def test_large_constant_override_is_restored_on_registration_failure(monkeypatch):
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints={42: Constraint(function=1e30, equality=Equality.EqualToZero)},
        sense=Sense.Minimize,
    )
    models = []

    def fail_registration(model, *args, **kwargs):
        models.append(model)
        assert model.getOptionValue("infinite_bound") == (
            highspy.HighsStatus.kOk,
            highspy.kHighsInf,
        )
        raise RuntimeError("constraint registration failed")

    monkeypatch.setattr(highspy.Highs, "addConstr", fail_registration)
    with pytest.raises(RuntimeError, match="constraint registration failed"):
        OMMXHighsAdapter(instance)
    [model] = models
    assert model.getOptionValue("infinite_bound") == highspy.Highs().getOptionValue(
        "infinite_bound"
    )
