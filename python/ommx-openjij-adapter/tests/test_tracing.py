from ommx.tracing import capture_trace
from ommx import DecisionVariable, Instance

from ommx_openjij_adapter import OMMXOpenJijSAAdapter


def _single_span(result, name):
    spans = [span for span in result.spans if span.name == name]
    assert len(spans) == 1
    return spans[0]


def _assert_sample_span_tree(result):
    root = _single_span(result, "ommx_trace_block")
    sample = _single_span(result, "sample")
    assert sample.parent_span_id == root.span_id
    for name in ("convert", "call", "decode"):
        assert _single_span(result, name).parent_span_id == sample.span_id


def test_direct_sample_emits_convert_call_decode_spans():
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

    _assert_sample_span_tree(result)
    assert not [span for span in result.spans if span.name == "prepare"]


def test_explicit_preparation_and_sample_are_sibling_operations():
    x = DecisionVariable.integer(0, lower=0, upper=3)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    source = instance.to_v2_bytes()

    with capture_trace() as result:
        preparation = OMMXOpenJijSAAdapter.prepare(instance)
        OMMXOpenJijSAAdapter.sample(
            preparation.input,
            num_reads=1,
            seed=0,
        )

    root = _single_span(result, "ommx_trace_block")
    prepare = _single_span(result, "prepare")
    sample = _single_span(result, "sample")
    assert prepare.parent_span_id == root.span_id
    assert sample.parent_span_id == root.span_id
    _assert_sample_span_tree(result)
    assert instance.to_v2_bytes() == source


def test_solve_delegates_to_sample_trace():
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

    _assert_sample_span_tree(result)
    assert not [span for span in result.spans if span.name == "prepare"]
