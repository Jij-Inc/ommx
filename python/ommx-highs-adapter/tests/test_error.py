import pytest

from ommx.v1 import Instance, DecisionVariable, Constraint
from ommx.adapter import InfeasibleDetected

from ommx_highs_adapter import OMMXHighsAdapter, OMMXHighsAdapterError


def test_error_nonlinear_objective():
    # Objective function: 2.3 * x * x
    x = DecisionVariable.continuous(0)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=2.3 * x * x,
        constraints=[],
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "HiGHS Adapter currently only supports linear problems" in str(e.value)


def test_error_nonlinear_constraint():
    # Objective function: 0
    # Constraint: 2.3 * x * x = 0
    x = DecisionVariable.continuous(1)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=0,  # constant 0
        constraints=[2.3 * x * x == 0],
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "HiGHS Adapter currently only supports linear problems" in str(e.value)


def test_error_infeasible_constant_equality_constraint():
    ommx_instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints=[
            Constraint(
                function=-1,
                equality=Constraint.EQUAL_TO_ZERO,
            ),
        ],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "Infeasible constant equality constraint" in str(e.value)


def test_error_infeasible_constant_inequality_constraint():
    ommx_instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints=[
            Constraint(
                function=1,
                equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
            ),
        ],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "Infeasible constant inequality constraint" in str(e.value)


def test_error_infeasible_model():
    x = DecisionVariable.continuous(1)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=0,
        constraints=[
            Constraint(
                function=x,
                equality=Constraint.EQUAL_TO_ZERO,
            ),
            Constraint(
                function=x - 1,
                equality=Constraint.EQUAL_TO_ZERO,
            ),
        ],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(InfeasibleDetected):
        OMMXHighsAdapter.solve(ommx_instance)
