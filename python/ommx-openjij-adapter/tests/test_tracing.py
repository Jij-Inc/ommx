from ommx.tracing import capture_trace
from ommx.v1 import DecisionVariable, Instance

from ommx_openjij_adapter import OMMXOpenJijSAAdapter


def test_sample_emits_convert_sample_decode_spans():
    x0 = DecisionVariable.binary(0)
    x1 = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints={},
        sense=Instance.MINIMIZE,
    )

    with capture_trace() as result:
        OMMXOpenJijSAAdapter.sample(instance, num_reads=1, seed=0)

    names = [s.name for s in result.spans]
    assert "convert" in names
    assert "sample" in names
    assert "decode" in names


def test_solve_emits_convert_sample_decode_spans():
    x0 = DecisionVariable.binary(0)
    x1 = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints={},
        sense=Instance.MINIMIZE,
    )

    with capture_trace() as result:
        OMMXOpenJijSAAdapter.solve(instance, num_reads=1, seed=0)

    names = [s.name for s in result.spans]
    assert "convert" in names
    assert "sample" in names
    assert "decode" in names
