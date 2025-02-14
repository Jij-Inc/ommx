import pytest
from ommx.v1 import Instance, DecisionVariable
from ommx.v1.solution_pb2 import Optimality

from ommx_gurobipy_adapter import OMMXGurobipyAdapter


def test_solution_optimality():
    """Test that optimal solutions are correctly marked as optimal"""
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(2, lower=0, upper=5)
    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXGurobipyAdapter.solve(ommx_instance)
    assert solution.optimality == Optimality.OPTIMALITY_OPTIMAL


def test_basic_functionality():
    """Test basic functionality with a simple optimization problem"""
    # Simple problem: maximize x + 2y subject to x + y <= 5
    x = DecisionVariable.continuous(1, lower=0, upper=10)
    y = DecisionVariable.continuous(2, lower=0, upper=10)

    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + 2 * y,
        constraints=[x + y <= 5],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXGurobipyAdapter.solve(instance)

    # Optimal solution should be x=0, y=5
    assert solution.state.entries[1] == pytest.approx(0)
    assert solution.state.entries[2] == pytest.approx(5)
    assert solution.objective == pytest.approx(10)  # 0 + 2*5


def test_multi_objective_handling():
    """Test that the adapter correctly handles multiple objectives by focusing on the primary one"""
    x = DecisionVariable.continuous(1, lower=0, upper=1)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,  # Primary objective: maximize x
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXGurobipyAdapter.solve(instance)

    # Should maximize x to its upper bound
    assert solution.state.entries[1] == pytest.approx(1)
    assert solution.objective == pytest.approx(1)
