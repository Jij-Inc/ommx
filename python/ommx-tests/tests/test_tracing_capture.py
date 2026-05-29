"""Tests for the script-side tracing API (``capture_trace`` + ``@traced``).

Complementary to :mod:`test_tracing_magic`, which exercises the
IPython cell magic, and :mod:`test_tracing_render`, which exercises
rendering completed trace results. These tests focus on:

* The context manager populates :class:`TraceResult.request` and doesn't
  swallow exceptions.
* ``@traced`` writes the trace on both the success and the exception
  paths so users who rely on the file never see it silently missing.
* Failure information survives: OTel records an ``ERROR`` status on
  the root span, and the traced exception still propagates to the caller.

The session-scoped ``TracerProvider`` from :mod:`conftest` is reused,
and a single module-level ``_TraceSpanCollector`` is attached to it
once and re-used across every test. The :func:`capture_collector`
fixture swaps ``_setup._COLLECTOR`` to that shared instance for the
duration of each test (so ``ensure_collector_installed`` returns it
rather than piling up new processors on the provider) and clears the
collector's internal state between tests via ``.shutdown()``.
"""

from __future__ import annotations

import json

import pytest
from opentelemetry.proto.collector.trace.v1.trace_service_pb2 import (
    ExportTraceServiceRequest,
)
from opentelemetry.proto.trace.v1.trace_pb2 import Status as ProtoStatus

from ommx.tracing import (
    TraceResult,
    capture_trace,
    traced,
)
from ommx.tracing import _setup
from ommx.tracing._collector import _TraceSpanCollector

from conftest import get_test_provider


# ---------------------------------------------------------------------------
# Shared collector fixture
# ---------------------------------------------------------------------------


_SESSION_COLLECTOR: _TraceSpanCollector | None = None


def _trace_id(span) -> int:
    return int.from_bytes(span.trace_id, byteorder="big")


@pytest.fixture
def capture_collector():
    """Attach a single collector to the session provider and clear its
    state between tests.

    Points ``_setup._COLLECTOR`` at the shared instance so
    ``ensure_collector_installed()`` returns it unchanged rather than
    creating a fresh ``_TraceSpanCollector`` + attaching a new
    ``SpanProcessor`` to the provider each test. Without that,
    processors would pile up across the suite.
    """
    global _SESSION_COLLECTOR
    if _SESSION_COLLECTOR is None:
        _SESSION_COLLECTOR = _TraceSpanCollector()
        get_test_provider().add_span_processor(_SESSION_COLLECTOR)
    # Reuse the shared collector for any ``capture_trace`` /
    # ``run_cell_with_trace`` call made during the test.
    _setup._COLLECTOR = _SESSION_COLLECTOR
    _SESSION_COLLECTOR.shutdown()  # drop state from previous test
    try:
        yield _SESSION_COLLECTOR
    finally:
        _SESSION_COLLECTOR.shutdown()
        # Hand the cache back to ``None`` so tests that explicitly
        # exercise ``ensure_collector_installed`` start from a clean
        # slate.
        _setup._COLLECTOR = None


# ---------------------------------------------------------------------------
# capture_trace: happy path
# ---------------------------------------------------------------------------


def test_capture_trace_populates_result_on_success(capture_collector):
    """``__exit__`` must fill in ``TraceResult.request`` before returning."""
    from opentelemetry import trace

    with capture_trace("example_op") as result:
        tracer = trace.get_tracer("ommx-capture-test")
        with tracer.start_as_current_span("inner"):
            pass

    # Root + inner span land in the collected list.
    assert sorted(span.name for span in result.spans) == ["example_op", "inner"]


def test_capture_trace_custom_span_name(capture_collector):
    """The caller can pick a descriptive root span name."""
    with capture_trace("build_qubo") as result:
        pass
    assert [span.name for span in result.spans] == ["build_qubo"]


def test_trace_result_otlp_protobuf_roundtrip(capture_collector):
    """Trace layers use OpenTelemetry's protobuf representation."""
    from opentelemetry import trace

    with capture_trace("protobuf_root") as result:
        tracer = trace.get_tracer("ommx-capture-test")
        with tracer.start_as_current_span("protobuf_child"):
            pass

    request = ExportTraceServiceRequest()
    request.ParseFromString(result.otlp_protobuf())
    names = {span.name for span in TraceResult(request=request).spans}
    assert names == {"protobuf_root", "protobuf_child"}

    restored = TraceResult.from_otlp_protobuf(result.otlp_protobuf())
    assert {span.name for span in restored.spans} == names


# ---------------------------------------------------------------------------
# capture_trace: exception path — must preserve info
# ---------------------------------------------------------------------------


def test_capture_trace_propagates_exception(capture_collector):
    """The managed block must not swallow exceptions — the caller
    relies on the failure to stop further work."""
    with pytest.raises(ValueError, match="boom"):
        with capture_trace() as _result:
            raise ValueError("boom")


def test_capture_trace_populates_result_on_exception(capture_collector):
    """Even when the block raised, ``result.request`` must be usable
    from an outer ``try``/``except`` so the caller can still inspect
    or save the trace — information is never dropped."""
    result: TraceResult | None = None
    try:
        with capture_trace("failing_op") as r:
            result = r
            raise ValueError("boom")
    except ValueError:
        pass

    assert result is not None
    assert result.spans, "TraceResult.spans must be populated on the exception path"
    # The root span sees the exception via OTel's default
    # record_exception + set_status(ERROR).
    root = next(s for s in result.spans if s.name == "failing_op")
    assert root.status.code == ProtoStatus.STATUS_CODE_ERROR


def test_capture_trace_uses_fresh_trace_id(capture_collector):
    """``start_as_current_span`` is handed an explicit empty Context
    so each block is its own root, not a child of any ambient span.
    Without the detach, spans from unrelated instrumentation could
    land in the captured list."""
    from opentelemetry import trace

    tracer = trace.get_tracer("ommx-capture-test.ambient")
    with tracer.start_as_current_span("ambient_parent") as ambient:
        with tracer.start_as_current_span("ambient_sibling"):
            pass
        with capture_trace("isolated") as result:
            pass
        ambient_trace_id = ambient.get_span_context().trace_id

    # Captured spans only contain the isolated root.
    spans = result.spans
    assert [s.name for s in spans] == ["isolated"]
    assert _trace_id(spans[0]) != ambient_trace_id


# ---------------------------------------------------------------------------
# @traced decorator
# ---------------------------------------------------------------------------


def test_traced_decorator_no_args_runs_and_returns(capture_collector):
    """``@traced`` without arguments traces the call and returns the
    wrapped function's result unchanged."""

    @traced
    def process(x, y):
        return x + y

    assert process(2, 3) == 5


def test_traced_decorator_with_args(capture_collector):
    """``@traced()`` with keyword args behaves identically to the
    no-arg form for the caller."""

    @traced(name="custom_name")
    def process():
        return 42

    assert process() == 42


def test_traced_decorator_writes_output_on_success(capture_collector, tmp_path):
    """``output=...`` writes the Chrome Trace JSON on the success path."""
    out = tmp_path / "success.json"

    @traced(name="success_op", output=out)
    def process():
        return 7

    assert process() == 7
    assert out.exists()
    payload = json.loads(out.read_text(encoding="utf-8"))
    names = {e["name"] for e in payload["traceEvents"]}
    assert "success_op" in names


def test_traced_decorator_writes_output_on_exception(capture_collector, tmp_path):
    """The exception path must still write the trace to disk —
    otherwise users who rely on the file for post-mortem analysis
    would find it missing exactly when they need it most."""
    out = tmp_path / "failure.json"

    @traced(name="failing_op", output=out)
    def process():
        raise ValueError("boom")

    with pytest.raises(ValueError, match="boom"):
        process()

    assert out.exists(), (
        "Trace must be written even when the decorated function raised — "
        "losing the trace on failure defeats the purpose of tracing."
    )
    payload = json.loads(out.read_text(encoding="utf-8"))
    names = {e["name"] for e in payload["traceEvents"]}
    assert "failing_op" in names


def test_traced_decorator_uses_function_qualname_by_default(
    capture_collector, tmp_path
):
    """If ``name`` is omitted the span is named after the function so
    traces from multiple decorated functions are easy to tell apart.

    Uses ``__qualname__`` rather than ``__name__`` so methods inside a
    class show up as ``ClassName.method_name`` (for a local function
    in a test, the qualname includes the enclosing-test prefix — we
    just check the function's base name appears in the event name).
    """
    out = tmp_path / "named.json"

    @traced(output=out)
    def build_qubo():
        return 1

    build_qubo()
    payload = json.loads(out.read_text(encoding="utf-8"))
    names = [e["name"] for e in payload["traceEvents"]]
    assert any("build_qubo" in n for n in names), (
        f"Span name should derive from the function. Got: {names}"
    )


def test_traced_decorator_supports_async_functions(capture_collector, tmp_path):
    """``async def`` must be traced end-to-end, not just coroutine creation.

    A plain sync wrapper would run the decorated call, return the
    coroutine object, close the capture window, and then the user
    would ``await`` the coroutine *outside* the capture — every span
    emitted by the awaited work would be dropped.
    """
    import asyncio

    out = tmp_path / "async.json"

    @traced(name="async_op", output=out)
    async def async_process():
        await asyncio.sleep(0)
        return 11

    result = asyncio.run(async_process())
    assert result == 11
    assert out.exists()
    payload = json.loads(out.read_text(encoding="utf-8"))
    names = {e["name"] for e in payload["traceEvents"]}
    # The root span from the ``capture_trace`` block is present —
    # this asserts the capture actually wrapped the awaited body, not
    # just the coroutine-creation moment.
    assert "async_op" in names


def test_traced_decorator_save_failure_does_not_mask_user_exception(
    capture_collector, tmp_path
):
    """If the wrapped function raised and ``save_chrome_trace`` also
    fails (e.g. disk full, read-only target, bad path), the user's
    original exception must win — that's the signal they care about.
    The save failure is swallowed in the exception path.
    """
    bad_path = tmp_path / "nonexistent-directory-that-will-be-a-file"
    bad_path.write_text("not a directory")
    # Path whose *parent* exists but is a file, so ``mkdir(parents=True)``
    # fails → ``save_chrome_trace`` raises.
    unusable_output = bad_path / "child.json"

    @traced(name="failing_with_bad_output", output=unusable_output)
    def process():
        raise ValueError("original user failure")

    with pytest.raises(ValueError, match="original user failure"):
        process()


def test_traced_decorator_save_failure_surfaces_on_success_path(
    capture_collector, tmp_path
):
    """On the success path, a broken ``output`` configuration should
    still be noticed — otherwise users never learn their trace writer
    was misconfigured until they go looking for the file.
    """
    bad_path = tmp_path / "file_not_dir"
    bad_path.write_text("not a directory")
    unusable_output = bad_path / "child.json"

    @traced(name="ok_but_bad_output", output=unusable_output)
    def process():
        return 1

    with pytest.raises((OSError, NotADirectoryError, FileNotFoundError)):
        process()


def test_traced_decorator_preserves_metadata(capture_collector):
    """``functools.wraps`` preserves ``__name__`` / ``__doc__`` —
    important for introspection tools and help()."""

    @traced(name="something_else")
    def process():
        """Do a thing."""
        return 0

    assert process.__name__ == "process"
    assert process.__doc__ == "Do a thing."


def test_trace_result_request_is_export_trace_service_request(capture_collector):
    with capture_trace() as result:
        pass
    assert isinstance(result.request, ExportTraceServiceRequest)
    assert result.spans
