import ommx_python_mip_adapter as adapter
from ommx.v1 import Instance, DecisionVariable


def test_empty():
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x], objective=x, sense=Instance.MINIMIZE, constraints=[]
    )
    result = adapter.solve(instance)
    assert result.HasField("solution")
