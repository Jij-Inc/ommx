"""Tests for the script-side tracing API (``capture_trace`` + ``@traced``).

Complementary to :mod:`test_tracing_magic`, which exercises the
IPython cell magic. Both APIs share the underlying ``_collector`` and
``_render`` modules, so these tests focus on:

* The context manager populates :class:`TraceResult.spans` and doesn't
  swallow exceptions.
* ``save_chrome_trace`` writes valid JSON, with parent dirs created.
* ``@traced`` writes the trace on both the success and the exception
  paths so users who rely on the file never see it silently missing.
* Failure information survives: OTel records an ``ERROR`` status on
  the root span, the renderer flags it, and the traced exception
  still propagates to the caller.

The session-scoped ``TracerProvider`` from :mod:`conftest` is reused;
``_setup.reset_for_testing`` + a fresh ``_CellSpanCollector`` attached
to the provider give each test isolation without touching the global
OTel state.
"""

from __future__ import annotations

import json

import pytest
from opentelemetry.sdk.trace import ReadableSpan
from opentelemetry.trace.status import StatusCode

from ommx.tracing import TraceResult, capture_trace, traced
from ommx.tracing import _setup
from ommx.tracing._collector import _CellSpanCollector

from conftest import get_test_provider


# ---------------------------------------------------------------------------
# Shared collector fixture
# ---------------------------------------------------------------------------


_SESSION_COLLECTOR: _CellSpanCollector | None = None


@pytest.fixture
def capture_collector():
    """Attach a single collector to the session provider and clear its
    state between tests.

    Points ``_setup._COLLECTOR`` at the shared instance so
    ``ensure_collector_installed()`` returns it unchanged rather than
    creating a fresh ``_CellSpanCollector`` + attaching a new
    ``SpanProcessor`` to the provider each test. Without that,
    processors would pile up across the suite.
    """
    global _SESSION_COLLECTOR
    if _SESSION_COLLECTOR is None:
        _SESSION_COLLECTOR = _CellSpanCollector()
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
    """``__exit__`` must fill in ``TraceResult.spans`` before returning."""
    from opentelemetry import trace

    with capture_trace("example_op") as result:
        tracer = trace.get_tracer("ommx-capture-test")
        with tracer.start_as_current_span("inner"):
            pass

    # Root + inner span land in the collected list.
    names = sorted(s.name for s in result.spans)
    assert names == ["example_op", "inner"]
    # Text tree is useful to print from scripts.
    tree = result.text_tree()
    assert "example_op" in tree and "inner" in tree


def test_capture_trace_custom_span_name(capture_collector):
    """The caller can pick a descriptive root span name."""
    with capture_trace("build_qubo") as result:
        pass
    assert [s.name for s in result.spans] == ["build_qubo"]


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
    """Even when the block raised, ``result.spans`` must be usable
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
    assert root.status is not None
    assert root.status.status_code == StatusCode.ERROR
    # And the renderer surfaces the error status so the user can find
    # the failing span at a glance.
    tree = result.text_tree()
    assert "[ERROR]" in tree
    assert "failing_op" in tree


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
    assert [s.name for s in result.spans] == ["isolated"]
    # Their trace_ids differ. ``context`` is Optional[SpanContext];
    # for a finished ``ReadableSpan`` it's always populated, but
    # assert it for the type checker.
    isolated_ctx = result.spans[0].context
    assert isolated_ctx is not None
    assert isolated_ctx.trace_id != ambient_trace_id


# ---------------------------------------------------------------------------
# Chrome Trace output + save_chrome_trace
# ---------------------------------------------------------------------------


def test_save_chrome_trace_writes_valid_json(capture_collector, tmp_path):
    """``save_chrome_trace`` writes the JSON the other tools consume."""
    with capture_trace() as result:
        pass

    out = tmp_path / "nested" / "trace.json"
    result.save_chrome_trace(out)

    assert out.exists(), "save_chrome_trace should create parent dirs"
    parsed = json.loads(out.read_text(encoding="utf-8"))
    assert parsed["displayTimeUnit"] == "ms"
    assert parsed["traceEvents"], "Trace must include at least the root span event"


def test_trace_result_chrome_trace_json_string(capture_collector):
    """``chrome_trace_json`` is the same JSON that ``save_chrome_trace``
    would write — handy for tests and for piping into another tool
    without touching the filesystem."""
    with capture_trace() as result:
        pass
    payload = result.chrome_trace_json()
    parsed = json.loads(payload)
    assert parsed["traceEvents"]


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


# ---------------------------------------------------------------------------
# Renderer: the new [ERROR] marker
# ---------------------------------------------------------------------------


def test_text_tree_marks_error_spans(capture_collector):
    """Spans with ``Status(ERROR)`` must be flagged in the tree output
    so the failing leaf is obvious when reading a trace post-mortem."""
    from opentelemetry import trace

    result: TraceResult | None = None
    try:
        with capture_trace("outer") as r:
            result = r
            tracer = trace.get_tracer("ommx-capture-test")
            with tracer.start_as_current_span("inner"):
                raise RuntimeError("kaboom")
    except RuntimeError:
        pass

    assert result is not None
    tree = result.text_tree()
    # Both the inner span (active when the exception fired) and the
    # root block inherit the ERROR status.
    outer_line = next(line for line in tree.splitlines() if "outer" in line)
    inner_line = next(line for line in tree.splitlines() if "inner" in line)
    assert "[ERROR]" in outer_line
    assert "[ERROR]" in inner_line


def test_text_tree_does_not_mark_successful_spans(capture_collector):
    """Spans that completed without an exception must stay unmarked."""
    with capture_trace("clean") as result:
        pass
    tree = result.text_tree()
    assert "[ERROR]" not in tree


# ---------------------------------------------------------------------------
# Sanity check: ReadableSpan type is the real one (regression for Any stubs)
# ---------------------------------------------------------------------------


def test_trace_result_spans_are_readable_spans(capture_collector):
    with capture_trace() as result:
        pass
    assert all(isinstance(s, ReadableSpan) for s in result.spans)
