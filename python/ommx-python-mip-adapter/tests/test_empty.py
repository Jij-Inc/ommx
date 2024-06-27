import ommx_python_mip_adapter as adapter
from ommx.v1 import Instance, DecisionVariable


def test_empty():
    x = DecisionVariable.binary(0)
    y = DecisionVariable.integer(1, lower=0, upper=10)
    z = DecisionVariable.binary(2)
    instance = Instance.from_components(
        decision_variables=[x, y, z],
        objective=x - y + z,
        sense=Instance.MINIMIZE,
        constraints=[x + z == 1],
    )
    result = adapter.solve(instance, relax=True)
    assert result.HasField("solution")
