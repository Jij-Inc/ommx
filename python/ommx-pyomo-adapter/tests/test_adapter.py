from ommx.v1 import Instance, DecisionVariable, Solution

from ommx_pyomo_adapter import OMMXPyomoAdapter


def test_solution_optimality():
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(2, lower=0, upper=5)
    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXPyomoAdapter.solve(ommx_instance)
    assert solution.optimality == Solution.OPTIMAL
