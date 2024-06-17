import pytest

from ommx.v1 import Instance, DecisionVariable
from ommx.testing import SingleFeasibleLPGenerator, DataType

import ommx_python_mip_adapter as adapter


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

    model = adapter.instance_to_model(ommx_instance_bytes)
    model.optimize()
    ommx_solution = adapter.model_to_solution(model, ommx_instance_bytes)
    expected_solution = generater.get_v1_state()
    assert ommx_solution.entries.keys() == expected_solution.entries.keys()
    for key in ommx_solution.entries.keys():
        assert ommx_solution.entries[key] == pytest.approx(
            expected_solution.entries[key]
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

    model = adapter.instance_to_model(ommx_instance)
    model.optimize()
    ommx_solution = adapter.model_to_solution(model, ommx_instance)

    assert ommx_solution.entries[1] == pytest.approx(3)
    assert ommx_solution.entries[2] == pytest.approx(3)
