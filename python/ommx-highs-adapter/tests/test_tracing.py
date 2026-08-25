from ommx.tracing import capture_trace
from ommx import DecisionVariable, Instance, OneHotConstraint, Solution
from ommx.adapter import DiagnosticsSink

from ommx_highs_adapter import OMMXHighsAdapter


def _single_span(result, name):
    spans = [span for span in result.spans if span.name == name]
    assert len(spans) == 1
    return spans[0]


def _assert_solve_span_tree(result):
    root = _single_span(result, "ommx_trace_block")
    solve = _single_span(result, "solve")
    assert solve.parent_span_id == root.span_id
    for name in ("convert", "call", "decode"):
        assert _single_span(result, name).parent_span_id == solve.span_id


def test_solve_emits_convert_call_decode_under_solve():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1] + x[2],
        constraints={},
        one_hot_constraints={0: OneHotConstraint(variables=x)},
        sense=Instance.MINIMIZE,
    )

    with capture_trace() as result:
        OMMXHighsAdapter.solve(instance)

    _assert_solve_span_tree(result)


def test_solve_without_preparation_emits_one_solve_span():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1] + x[2],
        constraints={0: x[0] + x[1] + x[2] <= 2},
        sense=Instance.MINIMIZE,
    )
    input_bytes = instance.to_v2_bytes()

    with capture_trace() as result:
        OMMXHighsAdapter.solve_without_preparation(instance)

    assert instance.to_v2_bytes() == input_bytes
    _assert_solve_span_tree(result)


def test_solve_dispatches_to_overridden_without_preparation():
    called = False

    class OverridingAdapter(OMMXHighsAdapter):
        @classmethod
        def solve_without_preparation(
            cls,
            ommx_instance: Instance,
            *,
            verbose: bool = False,
            diagnostics: DiagnosticsSink | None = None,
        ) -> Solution:
            nonlocal called
            called = True
            return super().solve_without_preparation(
                ommx_instance,
                verbose=verbose,
                diagnostics=diagnostics,
            )

    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MINIMIZE,
    )

    with capture_trace() as result:
        OverridingAdapter.solve(instance)

    assert called
    _single_span(result, "solve")


def test_manual_flow_emits_convert_and_decode_spans():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1] + x[2],
        constraints={0: x[0] + x[1] + x[2] <= 2},
        sense=Instance.MINIMIZE,
    )

    with capture_trace() as result:
        adapter = OMMXHighsAdapter(instance)
        model = adapter.solver_input
        model.run()
        adapter.decode(model)

    names = [span.name for span in result.spans]
    assert "convert" in names
    assert "decode" in names
