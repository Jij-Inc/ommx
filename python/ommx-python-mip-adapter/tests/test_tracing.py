from ommx.tracing import capture_trace
from ommx.v1 import DecisionVariable, Instance

from ommx_python_mip_adapter import OMMXPythonMIPAdapter


def _span_names(result) -> list[str]:
    return [
        span.name
        for resource_span in result.request.resource_spans
        for scope_span in resource_span.scope_spans
        for span in scope_span.spans
    ]


def test_solve_emits_convert_solve_decode_spans():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1] + x[2],
        constraints={0: x[0] + x[1] + x[2] <= 2},
        sense=Instance.MINIMIZE,
    )

    with capture_trace() as result:
        OMMXPythonMIPAdapter.solve(instance)

    names = _span_names(result)
    assert "convert" in names
    assert "solve" in names
    assert "decode" in names


def test_manual_flow_emits_convert_and_decode_spans():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1] + x[2],
        constraints={0: x[0] + x[1] + x[2] <= 2},
        sense=Instance.MINIMIZE,
    )

    with capture_trace() as result:
        adapter = OMMXPythonMIPAdapter(instance)
        model = adapter.solver_input
        model.optimize()
        adapter.decode(model)

    names = _span_names(result)
    assert "convert" in names
    assert "decode" in names
