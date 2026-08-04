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


def test_explicit_preparation_does_not_wrap_the_direct_sample_operation():
    x = DecisionVariable.integer(0, lower=0, upper=3)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    prepared: bytes | None = None
    with capture_trace() as result:
        instance.prepare(
            OMMXOpenJijSAAdapter.INPUT_CLASS,
            OMMXOpenJijSAAdapter.recommended_preparation_policy(),
        )
        prepared = instance.to_v2_bytes()
        OMMXOpenJijSAAdapter.sample(
            instance,
            num_reads=1,
            seed=0,
        )

    root = _single_span(result, "ommx_trace_block")
    sample = _single_span(result, "sample")
    assert sample.parent_span_id == root.span_id
    # Common Preparation is a Rust-owned Instance operation. Its span follows
    # the process RUST_LOG filter, unlike the Adapter's Python OTel spans. When
    # recorded, it is a sibling of the strict direct sample operation.
    for prepare in [span for span in result.spans if span.name == "prepare"]:
        assert prepare.parent_span_id == root.span_id
    _assert_sample_span_tree(result)
    assert prepared is not None
    assert instance.to_v2_bytes() == prepared


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
