"""End-to-end tests for ``ommx.tracing`` (the ``%%ommx_trace`` cell magic).

The magic is supposed to:

1. Wrap a cell in a single OTel root span.
2. Collect every span emitted during the cell (including Rust spans
   forwarded by ``pyo3-tracing-opentelemetry``).
3. Render a text tree and attach a Chrome Trace JSON download link.

These tests exercise the collector and renderers directly for focused
unit coverage, then drive the full magic through an ``IPython`` shell
to verify the integration end-to-end.

The session-scoped ``TracerProvider`` installed by :mod:`conftest` is
reused throughout: OTel refuses to swap providers once set, so tests
share the same one and distinguish runs by ``trace_id``.
"""

from __future__ import annotations

import base64
import json
import threading
import time
import urllib.parse

import pytest
from opentelemetry import trace

from ommx.tracing import _setup
from ommx.tracing._collector import _CellSpanCollector
from ommx.tracing._magic import run_cell_with_trace
from ommx.tracing._render import (
    chrome_trace_json,
    render_cell_output_html,
    render_text_tree,
    to_chrome_trace,
)

from conftest import get_test_provider


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


@pytest.fixture
def fresh_collector():
    """A collector attached to the session provider for the test's use.

    Each test gets its own collector instance so ``pop_trace`` does not
    have to guard against spans from previous tests. The collector is
    left attached for the session — we cannot remove processors from an
    SDK provider without reaching into private attributes, and the
    session exporter continues to receive spans regardless, so this only
    costs a little per-span work.
    """
    _setup.reset_for_testing()
    provider = get_test_provider()
    collector = _CellSpanCollector()
    provider.add_span_processor(collector)
    yield collector
    _setup.reset_for_testing()


def _run_and_collect(collector: _CellSpanCollector, cell_fn):
    """Run ``cell_fn`` inside a fresh root span and return its spans.

    Uses the global tracer so trace-context propagation matches what the
    real cell magic does.
    """
    tracer = trace.get_tracer("ommx-tracing-magic-test")
    with tracer.start_as_current_span("root") as root:
        trace_id = root.get_span_context().trace_id
        cell_fn(tracer)
    return collector.pop_trace(trace_id)


# ---------------------------------------------------------------------------
# Collector
# ---------------------------------------------------------------------------


def test_collector_groups_spans_by_trace_id(fresh_collector):
    """``pop_trace`` returns exactly the spans tagged with that ``trace_id``."""
    tracer = trace.get_tracer("ommx-tracing-magic-test")

    with tracer.start_as_current_span("cell_a") as a:
        trace_id_a = a.get_span_context().trace_id
        with tracer.start_as_current_span("child_a"):
            pass
    with tracer.start_as_current_span("cell_b") as b:
        trace_id_b = b.get_span_context().trace_id

    assert trace_id_a != trace_id_b
    spans_a = fresh_collector.pop_trace(trace_id_a)
    spans_b = fresh_collector.pop_trace(trace_id_b)
    assert sorted(s.name for s in spans_a) == ["cell_a", "child_a"]
    assert sorted(s.name for s in spans_b) == ["cell_b"]

    # Second pop for the same trace returns an empty list — the collector
    # must not accumulate state.
    assert fresh_collector.pop_trace(trace_id_a) == []


def test_collector_is_threadsafe(fresh_collector):
    """The collector is called from exporter threads; concurrent writes
    must not lose spans or corrupt the internal dict."""
    tracer = trace.get_tracer("ommx-tracing-magic-test")

    root_trace_ids: list[int] = []

    def worker():
        with tracer.start_as_current_span("worker_root") as root:
            root_trace_ids.append(root.get_span_context().trace_id)
            for _ in range(20):
                with tracer.start_as_current_span("work"):
                    time.sleep(0.0001)

    threads = [threading.Thread(target=worker) for _ in range(4)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    # One root span + 20 inner spans per worker = 21 per thread.
    total = sum(len(fresh_collector.pop_trace(tid)) for tid in root_trace_ids)
    assert total == 4 * 21


# ---------------------------------------------------------------------------
# Renderers
# ---------------------------------------------------------------------------


def test_render_text_tree_reflects_nesting(fresh_collector):
    """Children appear indented under their parent, not as siblings."""

    def cell(tracer):
        with tracer.start_as_current_span("outer"):
            with tracer.start_as_current_span("inner"):
                pass

    spans = _run_and_collect(fresh_collector, cell)
    tree = render_text_tree(spans)

    # ``root`` is the cell's root; ``outer`` is a child; ``inner`` is a
    # grandchild. The indent on ``inner`` must be strictly greater than
    # on ``outer`` for the tree to be useful.
    outer_line = next(line for line in tree.splitlines() if "outer" in line)
    inner_line = next(line for line in tree.splitlines() if "inner" in line)
    outer_indent = len(outer_line) - len(outer_line.lstrip())
    inner_indent = len(inner_line) - len(inner_line.lstrip())
    assert inner_indent > outer_indent


def test_render_text_tree_handles_empty():
    assert render_text_tree([]) == "(no spans)"


def test_chrome_trace_is_valid_json_with_X_events(fresh_collector):
    """Chrome-trace JSON must parse, every event has ``ph: 'X'``, and
    durations are in microseconds (positive integers)."""

    def cell(tracer):
        with tracer.start_as_current_span("work") as span:
            span.set_attribute("batch_size", 42)

    spans = _run_and_collect(fresh_collector, cell)
    payload = chrome_trace_json(spans)
    parsed = json.loads(payload)

    assert parsed["displayTimeUnit"] == "ms"
    events = parsed["traceEvents"]
    # Two events: the synthetic ``root`` and the inner ``work`` span.
    assert {e["name"] for e in events} == {"root", "work"}
    for ev in events:
        assert ev["ph"] == "X"
        assert isinstance(ev["ts"], int) and ev["ts"] > 0
        assert isinstance(ev["dur"], int) and ev["dur"] >= 1

    work = next(e for e in events if e["name"] == "work")
    assert work["args"]["batch_size"] == 42


def test_chrome_trace_skips_open_spans():
    """Spans without an ``end_time`` must be omitted — they would emit a
    zero-duration event and confuse Perfetto."""
    from typing import cast

    from opentelemetry.sdk.trace import ReadableSpan

    class _Fake:
        name = "still_running"
        start_time = 1_000_000_000
        end_time = None
        attributes: dict = {}

    # The renderer only touches ``start_time``/``end_time``/``attributes``/
    # ``name``; a duck-typed stand-in is enough. Cast through ``ReadableSpan``
    # to keep pyright quiet without materializing a full span.
    result = to_chrome_trace([cast(ReadableSpan, _Fake())])
    assert result["traceEvents"] == []


def test_cell_html_contains_download_link(fresh_collector):
    """The HTML blob must carry a base64 data URL the browser can download."""

    def cell(tracer):
        with tracer.start_as_current_span("work"):
            pass

    spans = _run_and_collect(fresh_collector, cell)
    html = render_cell_output_html(spans, download_filename="cell.json")

    assert "<pre>" in html
    assert 'download="cell.json"' in html

    # Extract the data URL payload and make sure it roundtrips as JSON.
    marker = 'href="data:application/json;base64,'
    start = html.index(marker) + len(marker)
    end = html.index('"', start)
    b64 = html[start:end]
    decoded = base64.b64decode(urllib.parse.unquote(b64)).decode("utf-8")
    parsed = json.loads(decoded)
    assert parsed["traceEvents"]


# ---------------------------------------------------------------------------
# Setup: installing the collector onto the session provider
# ---------------------------------------------------------------------------


def test_ensure_collector_installed_is_idempotent():
    """Repeated calls must return the same cached collector so we do not
    keep appending processors to the provider across magic invocations."""
    _setup.reset_for_testing()
    try:
        first = _setup.ensure_collector_installed()
        second = _setup.ensure_collector_installed()
        assert first is second
    finally:
        _setup.reset_for_testing()


def test_ensure_collector_does_not_replace_existing_processors():
    """Pre-existing span processors continue to see spans after the
    collector is installed."""
    from conftest import get_test_exporter

    exporter = get_test_exporter()
    exporter.clear()
    _setup.reset_for_testing()
    try:
        _setup.ensure_collector_installed()

        tracer = trace.get_tracer("ommx-tracing-magic-test")
        with tracer.start_as_current_span("verify"):
            pass

        get_test_provider().force_flush()
        assert any(s.name == "verify" for s in exporter.spans), (
            "The pre-existing session exporter stopped receiving spans "
            "after the cell collector was attached."
        )
    finally:
        _setup.reset_for_testing()


# ---------------------------------------------------------------------------
# End-to-end through an IPython shell
# ---------------------------------------------------------------------------


@pytest.fixture
def ipython_shell():
    from IPython.testing.globalipapp import get_ipython

    shell = get_ipython()
    assert shell is not None
    return shell


def test_run_cell_with_trace_exec_user_code(ipython_shell):
    """The cell body actually runs in the user namespace — the assignment
    becomes visible to subsequent cells."""
    _setup.reset_for_testing()
    try:
        ipython_shell.user_ns.pop("ommx_trace_test_sentinel", None)

        html, exc = run_cell_with_trace(
            ipython_shell,
            "ommx_trace_test_sentinel = 7",
        )

        assert exc is None
        assert ipython_shell.user_ns["ommx_trace_test_sentinel"] == 7
        assert "Download Chrome Trace JSON" in html
    finally:
        _setup.reset_for_testing()


def test_run_cell_with_trace_reports_cell_exceptions(ipython_shell):
    """A cell that raises still produces HTML output, and the exception
    is reported back to the caller so IPython can show a traceback."""
    _setup.reset_for_testing()
    try:
        html, exc = run_cell_with_trace(
            ipython_shell,
            "raise ValueError('boom')",
        )
        assert isinstance(exc, ValueError)
        assert "boom" in str(exc)
        # The root span closes even when the cell body raised, so its
        # name must appear in the rendered text tree.
        assert "ommx_trace_cell" in html
    finally:
        _setup.reset_for_testing()


def test_load_ipython_extension_registers_magic(ipython_shell):
    """After ``%load_ext ommx.tracing`` the shell knows about
    ``%%ommx_trace``. We check the magic registry directly rather than
    parsing cell output, to avoid depending on IPython's HTML
    rendering."""
    ipython_shell.extension_manager.load_extension("ommx.tracing")
    assert "ommx_trace" in ipython_shell.magics_manager.magics["cell"]
