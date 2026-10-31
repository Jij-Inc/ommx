import pytest
from pyomo.opt import TerminationCondition

from ommx_pyomo_adapter import (
    OMMXPyomoAdapterError,
    OMMXPyomoAdapter,
)

from ommx.v1 import Constraint, Instance, DecisionVariable, Polynomial


def test_error_polynomial_objective():
    # Objective function: 2.3 * x * x * x
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=Polynomial(terms={(1, 1, 1): 2.3}),
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXPyomoAdapterError) as e:
        OMMXPyomoAdapter(ommx_instance)
    assert "Objective function degree" in str(e.value)


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
    with pytest.raises(OMMXPyomoAdapterError) as e:
        OMMXPyomoAdapter(ommx_instance)
    assert "Constraints must be either constant, linear or quadratic." in str(e.value)


def test_error_not_solved_model():
    x = DecisionVariable.continuous(1)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    adapter = OMMXPyomoAdapter(instance)
    # Create a mock results object without solving
    from pyomo.opt import SolverResults

    results = SolverResults()
    results.solver.termination_condition = TerminationCondition.unknown
    # Don't solve the model, so variables have no values
    with pytest.raises(OMMXPyomoAdapterError) as e:
        adapter.decode_to_state(results)
    assert "Failed to decode state from results" in str(e.value)


def test_error_solver_not_available():
    x = DecisionVariable.continuous(1)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXPyomoAdapterError) as e:
        OMMXPyomoAdapter.solve(ommx_instance, solver_name="nonexistent_solver")
    assert "Solver 'nonexistent_solver' is not available" in str(e.value)


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
    with pytest.raises(OMMXPyomoAdapterError) as e:
        OMMXPyomoAdapter(ommx_instance)
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
    with pytest.raises(OMMXPyomoAdapterError) as e:
        OMMXPyomoAdapter(ommx_instance)
    assert "Infeasible constant constraint was found" in str(e.value)
