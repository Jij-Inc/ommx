import pytest
import pyscipopt

from ommx_pyscipopt_adapter import (
    OMMXPySCIPOptAdapterError,
    OMMXPySCIPOptAdapter,
)

from ommx.adapter import InfeasibleDetected
from ommx.v1 import Constraint, Instance, DecisionVariable, Polynomial


def test_error_polynomial_objective():
    # Objective function: 2.3 * x * x * x
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=Polynomial(terms={(1, 1, 1): 2.3}),
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        OMMXPySCIPOptAdapter(ommx_instance)
    assert "The objective function must be" in str(e.value)


def test_error_nonlinear_constraint():
    # Objective function: 0
    # Constraint: 2.3 * x * x * x = 0
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=0,
        constraints=[
            Constraint(
                function=Polynomial(terms={(1, 1, 1): 2.3}),
                equality=Constraint.EQUAL_TO_ZERO,
            ),
        ],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        OMMXPySCIPOptAdapter(ommx_instance)
    assert "Constraints must be either constant, linear or quadratic." in str(e.value)


def test_error_not_optimized_model():
    model = pyscipopt.Model()
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        OMMXPySCIPOptAdapter(instance).decode_to_state(model)
    assert "The model may not be optimized." in str(e.value)


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
        OMMXPySCIPOptAdapter.solve(ommx_instance)


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
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        OMMXPySCIPOptAdapter(ommx_instance)
    assert "Infeasible constant constraint was found" in str(e.value)


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
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        OMMXPySCIPOptAdapter(ommx_instance)
    assert "Infeasible constant constraint was found" in str(e.value)
