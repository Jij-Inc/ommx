import pytest

from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.decision_variables_pb2 import DecisionVariable, Bound
from ommx.v1.function_pb2 import Function
from ommx.v1.instance_pb2 import Instance as _Instance
from ommx.v1.linear_pb2 import Linear
from ommx.v1.solution_pb2 import State
from ommx.v1 import Instance
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
    ommx_instance = Instance(
        _Instance(
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
                    equality=Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO,
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
                    equality=Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO,
                ),
            ],
        )
    )

    model = adapter.instance_to_model(ommx_instance)
    model.optimize()
    ommx_solution = adapter.model_to_solution(model, ommx_instance)

    assert ommx_solution.entries[1] == pytest.approx(3)
    assert ommx_solution.entries[2] == pytest.approx(3)
