from ommx.v1 import Instance, DecisionVariable, Solution

from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


def test_solution_optimality():
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(2, lower=0, upper=5)
    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXPySCIPOptAdapter.solve(ommx_instance)
    assert solution.optimality == Solution.OPTIMAL
