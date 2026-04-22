"""End-to-end tests for the Rust-to-Python OpenTelemetry bridge.

Verifies that:

1. Rust ``tracing`` spans emitted from instrumented entry points are
   forwarded to Python's configured ``TracerProvider``.
2. The Python-side parent trace context is propagated into Rust, so
   Rust spans become descendants of the Python parent span.
3. Events emitted via ``tracing::info!`` inside an instrumented Rust
   function surface as span events on the wrapping Rust span.

The session-scoped OTel provider is set up in :mod:`conftest`.
"""

from __future__ import annotations

from opentelemetry import trace
from opentelemetry.sdk.trace import ReadableSpan

from ommx.v1 import (
    DecisionVariable,
    Equality,
    IndicatorConstraint,
    Instance,
)

from conftest import get_test_exporter, get_test_provider


def _instance_with_indicator() -> Instance:
    """Instance with one indicator constraint so ``reduce_capabilities``
    has real work to do and emits the expected INFO event."""
    x = DecisionVariable.continuous(0, lower=0, upper=5)
    y = DecisionVariable.binary(1)
    ic = IndicatorConstraint(
        indicator_variable=y,
        function=DecisionVariable.continuous(0, lower=0, upper=5) - 2,
        equality=Equality.LessThanOrEqualToZero,
    )
    return Instance.from_components(
        decision_variables=[x, y],
        objective=x,
        constraints={},
        indicator_constraints={7: ic},
        sense=Instance.MINIMIZE,
    )


def _is_descendant_of(
    span: ReadableSpan,
    ancestor_span_id: int,
    span_by_id: dict[int, ReadableSpan],
) -> bool:
    visited: set[int] = set()
    current = span
    while current.parent is not None:
        parent_ctx = current.parent
        if parent_ctx.span_id == ancestor_span_id:
            return True
        if parent_ctx.span_id in visited:
            break
        visited.add(parent_ctx.span_id)
        parent_span = span_by_id.get(parent_ctx.span_id)
        if parent_span is None:
            break
        current = parent_span
    return False


def test_rust_spans_are_forwarded() -> None:
    """An instrumented Rust entry point must emit at least one span."""
    exporter = get_test_exporter()
    provider = get_test_provider()
    exporter.clear()

    instance = _instance_with_indicator()
    instance.reduce_capabilities(set())

    provider.force_flush()
    rust_spans = [s for s in exporter.spans if s.name == "reduce_capabilities"]
    assert len(rust_spans) >= 1, (
        "Expected at least one Rust 'reduce_capabilities' span, "
        f"got: {[s.name for s in exporter.spans]}"
    )


def test_trace_context_propagation() -> None:
    """Rust spans adopt the Python parent's trace_id and become descendants."""
    exporter = get_test_exporter()
    provider = get_test_provider()
    exporter.clear()

    tracer = trace.get_tracer("ommx-tracing-test")
    instance = _instance_with_indicator()

    with tracer.start_as_current_span("python_parent") as parent_span:
        parent_trace_id = parent_span.get_span_context().trace_id
        parent_span_id = parent_span.get_span_context().span_id
        instance.reduce_capabilities(set())

    provider.force_flush()

    all_spans = list(exporter.spans)
    python_parents = [s for s in all_spans if s.name == "python_parent"]
    rust_spans = [s for s in all_spans if s.name != "python_parent"]

    assert len(python_parents) == 1
    assert len(rust_spans) >= 1, "Expected at least one Rust span under python_parent"

    for s in rust_spans:
        assert s.context is not None
        assert s.context.trace_id == parent_trace_id, (
            f"Rust span {s.name} has trace_id "
            f"{format(s.context.trace_id, '032x')}, "
            f"expected {format(parent_trace_id, '032x')}"
        )

    span_by_id: dict[int, ReadableSpan] = {
        s.context.span_id: s for s in all_spans if s.context is not None
    }
    descendants = [
        s for s in rust_spans if _is_descendant_of(s, parent_span_id, span_by_id)
    ]
    assert len(descendants) >= 1, (
        "No Rust span is a descendant of python_parent. "
        f"Rust span parents: "
        f"{[format(s.parent.span_id, '016x') if s.parent else 'None' for s in rust_spans]}"
    )


def test_tracing_info_event_captured_on_rust_span() -> None:
    """``tracing::info!`` inside ``reduce_capabilities`` becomes a span event
    on the Rust span, not a separate span."""
    exporter = get_test_exporter()
    provider = get_test_provider()
    exporter.clear()

    instance = _instance_with_indicator()
    instance.reduce_capabilities(set())

    provider.force_flush()
    rust_spans = [s for s in exporter.spans if s.name == "reduce_capabilities"]
    assert rust_spans, "reduce_capabilities span was not exported"

    events_across_spans = [event for s in rust_spans for event in s.events]
    matched = [
        e
        for e in events_across_spans
        if "Indicator" in e.name and "converted to regular constraints" in e.name
    ]
    assert matched, (
        "Expected the INFO event from Rust to appear as a span event. "
        f"Got events: {[e.name for e in events_across_spans]}"
    )
