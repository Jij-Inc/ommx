import ommx_python_mip_adapter as adapter
from ommx.v1 import Instance, DecisionVariable, Linear
import pytest


@pytest.mark.skip(
    reason="This test causes a segfault due to a bug in the Python-MIP fixed in https://github.com/coin-or/python-mip/pull/237, which is not yet released."
)
def test_constant_constraint_feasible():
    x = DecisionVariable.continuous(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[
            # 1 >= 0 is always true
            Linear(terms={}, constant=1) >= 0,
            x <= 1,
        ],
        sense=Instance.MAXIMIZE,
    )
    result = adapter.solve(instance)
    assert result.HasField("solution")

    solution = result.solution
    assert solution.state.entries == {0: 1.0}
    assert solution.objective == 1.0

    assert len(solution.evaluated_constraints) == 2


@pytest.mark.skip(
    reason="This test causes a segfault due to a bug in the Python-MIP fixed in https://github.com/coin-or/python-mip/pull/237, which is not yet released."
)
def test_constant_constraint_infeasible():
    x = DecisionVariable.continuous(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[
            # -1 >= 0 is always false
            Linear(terms={}, constant=-1) >= 0,
            x <= 1,
        ],
        sense=Instance.MAXIMIZE,
    )
    result = adapter.solve(instance)
    assert result.HasField("infeasible")
