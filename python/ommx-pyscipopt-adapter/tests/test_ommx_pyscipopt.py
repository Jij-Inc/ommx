from ommx.v1 import Instance, DecisionVariable
from ommx.v1.solution_pb2 import Optimality

import ommx_pyscipopt_adapter as adapter


def test_solution_optimality():
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(1, lower=0, upper=5)
    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    model = adapter.instance_to_model(ommx_instance)
    model.optimize()
    solution = adapter.model_to_solution(model, ommx_instance)
    assert solution.optimality == Optimality.OPTIMALITY_OPTIMAL
