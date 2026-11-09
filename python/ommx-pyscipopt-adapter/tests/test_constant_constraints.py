import sys
from unittest.mock import Mock

import pytest
import pyscipopt

from ommx import (
    Constraint,
    DecisionVariable,
    Equality,
    InfeasibleOrUnboundedDetected,
    Instance,
    Sense,
)
from ommx.adapter import InfeasibleDetected, UnboundedDetected
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


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
        active_constraints={42: Constraint(function=constant, equality=equality)},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()
    assert OMMXPySCIPOptAdapter.check_applicability(instance).is_member
    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input
    default_model = pyscipopt.Model()
    assert model.getParam("misc/allowstrongdualreds") == default_model.getParam(
        "misc/allowstrongdualreds"
    )
    assert model.getStatus() == "unknown"
    assert model.getNConss() == 1

    model.optimize()
    if feasible:
        assert model.getStatus() == "optimal"
        state = adapter.decode_to_state(model)
        assert state.entries == ({11: 0.0} if variable_mode == "active" else {})
        solution = adapter.decode(model)
        assert solution.feasible()
        assert solution.objective == 7
    else:
        assert model.getStatus() == "infeasible"
        with pytest.raises(InfeasibleDetected):
            adapter.decode(model)

    for solve in (
        OMMXPySCIPOptAdapter.solve,
        OMMXPySCIPOptAdapter.solve_without_preparation,
    ):
        if feasible:
            solution = solve(instance)
            assert solution.feasible()
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
        active_constraints={42: Constraint(function=constant, equality=equality)},
        sense=Sense.Minimize,
    )
    feasible = constant == 0 if equality == Equality.EqualToZero else constant <= 0
    expected_exception = (
        UnboundedDetected if feasible else InfeasibleOrUnboundedDetected
    )
    adapter = OMMXPySCIPOptAdapter(instance)
    model = adapter.solver_input
    model.optimize()
    # With SCIP's default reductions this fixture is unresolved when the
    # constant row is false. Decoding must preserve that distinction.
    assert model.getStatus() == ("unbounded" if feasible else "inforunbd")
    with pytest.raises(expected_exception):
        adapter.decode(model)
    with pytest.raises(expected_exception):
        OMMXPySCIPOptAdapter.solve(instance)


def test_decode_unresolved_status_does_not_solve_or_claim_infeasibility():
    instance = Instance.from_components(
        decision_variables=[], objective=0, active_constraints={}, sense=Sense.Minimize
    )
    adapter = OMMXPySCIPOptAdapter(instance)
    model = Mock(spec=pyscipopt.Model)
    model.getStatus.return_value = "inforunbd"
    with pytest.raises(InfeasibleOrUnboundedDetected, match="status: inforunbd"):
        adapter.decode(model)
    model.optimize.assert_not_called()
    model.getBestSol.assert_not_called()


@pytest.mark.parametrize(
    "equality", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
@pytest.mark.parametrize("tolerance,feasible", [(1e-7, False), (1e-4, True)])
def test_constant_constraint_uses_solver_tolerance(equality, tolerance, feasible):
    instance = Instance.from_components(
        decision_variables=[],
        objective=7,
        active_constraints={42: Constraint(function=1e-5, equality=equality)},
        sense=Sense.Minimize,
    )
    assert OMMXPySCIPOptAdapter.check_applicability(instance).is_member
    model = OMMXPySCIPOptAdapter(instance).solver_input
    model.setRealParam("numerics/feastol", tolerance)
    model.optimize()
    assert model.getStatus() == ("optimal" if feasible else "infeasible")
