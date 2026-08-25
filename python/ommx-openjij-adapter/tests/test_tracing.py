from typing import Any

from ommx.tracing import capture_trace
from ommx import DecisionVariable, Instance, SampleSet, Solution

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


def test_easy_sample_uses_single_sample_span():
    x0 = DecisionVariable.binary(0)
    x1 = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints={},
        sense=Instance.MAXIMIZE,
    )

    with capture_trace() as result:
        OMMXOpenJijSAAdapter.sample(instance, num_reads=1, seed=0)

    _assert_sample_span_tree(result)


def test_sample_without_preparation_uses_single_span_and_preserves_input():
    x0 = DecisionVariable.binary(0)
    x1 = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints={},
        sense=Instance.MINIMIZE,
    )
    before = instance.to_v2_bytes()

    with capture_trace() as result:
        OMMXOpenJijSAAdapter.sample_without_preparation(
            instance,
            num_reads=1,
            seed=0,
        )

    _assert_sample_span_tree(result)
    assert instance.to_v2_bytes() == before


def test_easy_solve_nests_sampling_inside_solve():
    x0 = DecisionVariable.binary(0)
    x1 = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints={},
        sense=Instance.MAXIMIZE,
    )

    with capture_trace() as result:
        OMMXOpenJijSAAdapter.solve(instance, num_reads=1, seed=0)

    root = _single_span(result, "ommx_trace_block")
    solve = _single_span(result, "solve")
    sample = _single_span(result, "sample")
    assert solve.parent_span_id == root.span_id
    assert sample.parent_span_id == solve.span_id
    for name in ("convert", "call", "decode"):
        assert _single_span(result, name).parent_span_id == sample.span_id


def test_easy_apis_dispatch_prepared_copies_to_public_strict_overrides():
    sample_inputs: list[Instance] = []
    solve_inputs: list[Instance] = []

    class OverridingAdapter(OMMXOpenJijSAAdapter):
        @classmethod
        def sample_without_preparation(
            cls,
            ommx_instance: Instance,
            **kwargs: Any,
        ) -> SampleSet:
            sample_inputs.append(ommx_instance)
            return super().sample_without_preparation(ommx_instance, **kwargs)

        @classmethod
        def solve_without_preparation(
            cls,
            ommx_instance: Instance,
            **kwargs: Any,
        ) -> Solution:
            solve_inputs.append(ommx_instance)
            return super().solve_without_preparation(ommx_instance, **kwargs)

    x = DecisionVariable.binary(0)
    source = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )

    OverridingAdapter.sample(source, num_reads=1, seed=0)
    OverridingAdapter.solve(source, num_reads=1, seed=0)

    [sample_input, solve_sample_input] = sample_inputs
    [solve_input] = solve_inputs
    assert sample_input is not source
    assert solve_input is not source
    assert solve_sample_input is solve_input
    assert sample_input.sense == Instance.MINIMIZE
    assert solve_input.sense == Instance.MINIMIZE
