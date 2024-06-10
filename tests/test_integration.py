import pytest

from ommx.v1.constraint_pb2 import Constraint
from ommx.v1.decision_variables_pb2 import DecisionVariable, Bound
from ommx.v1.function_pb2 import Function
from ommx.v1.instance_pb2 import Instance
from ommx.v1.linear_pb2 import Linear
from ommx.v1.solution_pb2 import SolutionList
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

    ommx_solution_obj = SolutionList.FromString(ommx_solution)
    expected_solution_obj = SolutionList.FromString(generater.get_v1_solution())

    # Check the number of solutions
    assert len(ommx_solution_obj.solutions) == 1

    actual_entries = ommx_solution_obj.solutions[0].entries
    expected_entries = expected_solution_obj.solutions[0].entries

    # Check the solution of each decision variable
    for key, actual_value in actual_entries.items():
        expected_value = expected_entries[key]
        assert actual_value == pytest.approx(expected_value)


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
    ommx_instance = Instance(
        decision_variables=[
            DecisionVariable(
                id=1,
                kind=DecisionVariable.KIND_INTEGER,
                bound=Bound(
                    lower=LOWER_BOUND,
                    upper=UPPER_BOUND,
                ),
            ),
            DecisionVariable(
                id=2,
                kind=DecisionVariable.KIND_CONTINUOUS,
                bound=Bound(
                    lower=LOWER_BOUND,
                    upper=UPPER_BOUND,
                ),
            ),
        ],
        objective=Function(
            linear=Linear(
                terms=[
                    Linear.Term(
                        id=1,
                        coefficient=-1,
                    ),
                    Linear.Term(
                        id=2,
                        coefficient=-1,
                    ),
                ],
            )
        ),
        constraints=[
            Constraint(
                function=Function(
                    linear=Linear(
                        terms=[
                            Linear.Term(
                                id=1,
                                coefficient=3,
                            ),
                            Linear.Term(
                                id=2,
                                coefficient=-1,
                            ),
                        ],
                        constant=-6,
                    ),
                ),
                equality=Constraint.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO,
            ),
            Constraint(
                function=Function(
                    linear=Linear(
                        terms=[
                            Linear.Term(
                                id=1,
                                coefficient=-1,
                            ),
                            Linear.Term(
                                id=2,
                                coefficient=3,
                            ),
                        ],
                        constant=-6,
                    ),
                ),
                equality=Constraint.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO,
            ),
        ],
    )
    ommx_instance_bytes = ommx_instance.SerializeToString()

    model = adapter.instance_to_model(ommx_instance_bytes)
    model.optimize()
    ommx_solution = adapter.model_to_solution(model, ommx_instance_bytes)

    ommx_solution_obj = SolutionList.FromString(ommx_solution)

    assert len(ommx_solution_obj.solutions) == 1

    actual_entries = ommx_solution_obj.solutions[0].entries
    assert actual_entries[1] == pytest.approx(3)
    assert actual_entries[2] == pytest.approx(3)
