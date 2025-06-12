import pytest

from ommx.v1 import Instance, DecisionVariable, Solution
from ommx.testing import SingleFeasibleLPGenerator, DataType

from ommx_python_mip_adapter import OMMXPythonMIPAdapter


@pytest.mark.parametrize(
    "generater",
    [
        SingleFeasibleLPGenerator(10, DataType.INT),
        SingleFeasibleLPGenerator(10, DataType.FLOAT),
    ],
)
def test_integration_lp(generater):
    # Objective function: 0
    # Constraints:
    #     A @ x = b    (A: regular matrix, b: constant vector)
    ommx_instance_bytes = generater.get_v1_instance()

    adapter = OMMXPythonMIPAdapter(ommx_instance_bytes)
    model = adapter.solver_input
    model.optimize()
    ommx_state = adapter.decode_to_state(model)
    expected_solution = generater.get_v1_state()
    assert ommx_state.entries.keys() == expected_solution.entries.keys()
    for key in ommx_state.entries.keys():
        assert ommx_state.entries[key] == pytest.approx(
            expected_solution.entries[key], abs=1e-6
        )


def test_integration_milp():
    # Objective function: - x1 - x2
    # Constraints:
    #     3x1 - x2 - 6 <= 0
    #     -x1 + 3x2 - 6 <= 0
    #     0 <= x1 <= 10    (x: integer)
    #     0 <= x2 <= 10    (x: continuous)
    # Optimal solution: x1 = 3, x2 = 3
    LOWER_BOUND = 0
    UPPER_BOUND = 10
    x1 = DecisionVariable.integer(1, lower=LOWER_BOUND, upper=UPPER_BOUND)
    x2 = DecisionVariable.continuous(2, lower=LOWER_BOUND, upper=UPPER_BOUND)
    ommx_instance = Instance.from_components(
        decision_variables=[x1, x2],
        objective=-x1 - x2,
        constraints=[
            3 * x1 - x2 <= 6,
            -x1 + 3 * x2 <= 6,
        ],
        sense=Instance.MINIMIZE,
    )

    adapter = OMMXPythonMIPAdapter(ommx_instance)
    model = adapter.solver_input
    model.optimize()
    ommx_state = adapter.decode_to_state(model)

    assert ommx_state.entries[1] == pytest.approx(3)
    assert ommx_state.entries[2] == pytest.approx(3)


def test_solution_optimality():
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(2, lower=0, upper=5)
    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXPythonMIPAdapter.solve(ommx_instance)
    assert solution.optimality == Solution.OPTIMAL
