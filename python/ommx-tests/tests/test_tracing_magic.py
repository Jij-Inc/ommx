"""End-to-end tests for ``ommx.tracing`` (the ``%%ommx_trace`` cell magic).

The magic is supposed to:

1. Wrap a cell in a single OTel root span.
2. Collect only that cell's spans (including Rust spans forwarded by
   ``pyo3-tracing-opentelemetry``).
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
# Fixtures
# ---------------------------------------------------------------------------


# A single collector is attached to the session provider and reused by every
# test to avoid piling ``SpanProcessor`` instances on the provider each run.
# The collector only retains spans for traces opened via ``begin_capture``,
# so between-test state only lives inside ``_active_traces`` /
# ``_spans_by_trace`` — both cleared in the fixture below.
_SESSION_COLLECTOR: _CellSpanCollector | None = None


@pytest.fixture
def cell_collector():
    """Return a session-wide collector with per-test state cleared.

    Points ``_setup._COLLECTOR`` at the shared instance so
    ``ensure_collector_installed()`` (called indirectly by
    ``capture_trace`` / ``run_cell_with_trace``) returns it unchanged
    rather than creating a fresh ``_CellSpanCollector`` + attaching a
    new ``SpanProcessor`` every test. Without that, processors would
    pile up across the suite.
    """
    global _SESSION_COLLECTOR
    if _SESSION_COLLECTOR is None:
        _SESSION_COLLECTOR = _CellSpanCollector()
        get_test_provider().add_span_processor(_SESSION_COLLECTOR)
    _setup._COLLECTOR = _SESSION_COLLECTOR
    _SESSION_COLLECTOR.shutdown()  # drop state from previous test
    try:
        yield _SESSION_COLLECTOR
    finally:
        _SESSION_COLLECTOR.shutdown()
        _setup._COLLECTOR = None


def _run_and_collect(collector: _CellSpanCollector, cell_fn):
    """Run ``cell_fn`` under a root span, using ``begin/end_capture``."""
    tracer = trace.get_tracer("ommx-tracing-magic-test")
    with tracer.start_as_current_span("root") as root:
        trace_id = root.get_span_context().trace_id
        collector.begin_capture(trace_id)
        cell_fn(tracer)
    return collector.end_capture(trace_id)


# ---------------------------------------------------------------------------
# Collector
# ---------------------------------------------------------------------------


def test_collector_captures_only_active_traces(cell_collector):
    """Spans outside a ``begin_capture`` window are dropped immediately —
    this is the guard that keeps memory bounded in long-lived notebooks."""
    tracer = trace.get_tracer("ommx-tracing-magic-test")

    # Emit a span with no capture open. It must not be retained.
    with tracer.start_as_current_span("background"):
        pass

    # Now open a capture and emit a span — this one should be kept.
    with tracer.start_as_current_span("cell") as cell:
        trace_id = cell.get_span_context().trace_id
        cell_collector.begin_capture(trace_id)
        with tracer.start_as_current_span("inner"):
            pass

    captured = cell_collector.end_capture(trace_id)
    captured_names = sorted(s.name for s in captured)
    # ``background`` was dropped; ``cell`` + ``inner`` were kept.
    assert captured_names == ["cell", "inner"]
    assert not cell_collector._spans_by_trace  # noqa: SLF001 - test-only reach-in


def test_collector_pop_is_single_use(cell_collector):
    """``end_capture`` removes the entries and discards the trace_id from
    the active set, so a second call returns empty."""
    tracer = trace.get_tracer("ommx-tracing-magic-test")

    with tracer.start_as_current_span("cell") as cell:
        trace_id = cell.get_span_context().trace_id
        cell_collector.begin_capture(trace_id)

    first = cell_collector.end_capture(trace_id)
    second = cell_collector.end_capture(trace_id)
    assert len(first) == 1
    assert second == []


def test_collector_is_threadsafe(cell_collector):
    """``on_end`` is called from exporter threads; concurrent captures
    must not lose spans or corrupt internal state."""
    tracer = trace.get_tracer("ommx-tracing-magic-test")
    captured: list[int] = []

    def worker():
        with tracer.start_as_current_span("worker_root") as root:
            tid = root.get_span_context().trace_id
            cell_collector.begin_capture(tid)
            for _ in range(20):
                with tracer.start_as_current_span("work"):
                    time.sleep(0.0001)
        captured.append(len(cell_collector.end_capture(tid)))

    threads = [threading.Thread(target=worker) for _ in range(4)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    # Each worker sees its root + 20 inner = 21 spans.
    assert captured == [21] * 4


# ---------------------------------------------------------------------------
# Renderers
# ---------------------------------------------------------------------------


def test_render_text_tree_reflects_nesting(cell_collector):
    """Children appear indented under their parent, not as siblings."""

    def cell(tracer):
        with tracer.start_as_current_span("outer"):
            with tracer.start_as_current_span("inner"):
                pass

    spans = _run_and_collect(cell_collector, cell)
    tree = render_text_tree(spans)

    outer_line = next(line for line in tree.splitlines() if "outer" in line)
    inner_line = next(line for line in tree.splitlines() if "inner" in line)
    outer_indent = len(outer_line) - len(outer_line.lstrip())
    inner_indent = len(inner_line) - len(inner_line.lstrip())
    assert inner_indent > outer_indent


def test_render_text_tree_handles_empty():
    assert render_text_tree([]) == "(no spans)"


def test_chrome_trace_is_valid_json_with_X_events(cell_collector):
    """Chrome-trace JSON must parse, every event has ``ph: 'X'``, and
    durations are in microseconds (positive integers)."""

    def cell(tracer):
        with tracer.start_as_current_span("work") as span:
            span.set_attribute("batch_size", 42)

    spans = _run_and_collect(cell_collector, cell)
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


def test_cell_html_contains_download_link(cell_collector):
    """The HTML blob must carry a base64 data URL the browser can download."""

    def cell(tracer):
        with tracer.start_as_current_span("work"):
            pass

    spans = _run_and_collect(cell_collector, cell)
    html = render_cell_output_html(spans, download_filename="cell.json")

    assert "<pre>" in html
    assert 'download="cell.json"' in html

    marker = 'href="data:application/json;base64,'
    start = html.index(marker) + len(marker)
    end = html.index('"', start)
    b64 = html[start:end]
    decoded = base64.b64decode(urllib.parse.unquote(b64)).decode("utf-8")
    parsed = json.loads(decoded)
    assert parsed["traceEvents"]


def test_cell_html_escapes_download_filename_for_attribute():
    """A filename containing a quote must not break out of the
    ``download`` attribute — otherwise arbitrary HTML could be injected."""
    html = render_cell_output_html([], download_filename='"><script>alert(1)</script>')
    assert "<script>" not in html
    # Raw quote must not terminate the attribute prematurely.
    assert 'download=""><script>' not in html


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
    """A cell that raises still produces HTML output. The caller
    (``register_magic``) is responsible for re-raising so failure
    semantics propagate to outer automation; :func:`run_cell_with_trace`
    itself surfaces the exception on the return tuple."""
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


def test_magic_reraises_cell_exception(ipython_shell):
    """The registered ``%%ommx_trace`` magic must propagate exceptions
    from the cell body; otherwise notebook automation
    (``nbconvert --execute``, papermill) would silently treat failed
    traced cells as successful."""
    import pytest

    _setup.reset_for_testing()
    try:
        ipython_shell.extension_manager.load_extension("ommx.tracing")
        magic_fn = ipython_shell.magics_manager.magics["cell"]["ommx_trace"]
        with pytest.raises(ValueError, match="boom"):
            magic_fn("", "raise ValueError('boom')")
    finally:
        _setup.reset_for_testing()


def test_load_ipython_extension_registers_magic(ipython_shell):
    """After ``%load_ext ommx.tracing`` the shell knows about
    ``%%ommx_trace``. We check the magic registry directly rather than
    parsing cell output, to avoid depending on IPython's HTML
    rendering."""
    ipython_shell.extension_manager.load_extension("ommx.tracing")
    assert "ommx_trace" in ipython_shell.magics_manager.magics["cell"]


def test_run_cell_with_trace_starts_a_fresh_trace(ipython_shell, cell_collector):
    """The cell root span must *not* inherit an ambient trace context.

    If ``start_as_current_span`` is called without detaching from the
    current context, any ambient span (from another extension,
    instrumentation library, or leftover ``with`` block in the user
    namespace) bleeds into the cell's ``trace_id`` and the collector
    captures unrelated spans. This test installs an ambient span,
    emits a sibling span under it, runs a traced cell, and asserts
    the cell's rendered tree contains only the cell's own spans.
    """
    _setup.reset_for_testing()
    try:
        outer_tracer = trace.get_tracer("ommx-tracing-magic-test.ambient")
        with outer_tracer.start_as_current_span("ambient_parent") as ambient:
            ambient_trace_id = ambient.get_span_context().trace_id
            cell_collector.begin_capture(ambient_trace_id)
            # A sibling span on the ambient trace — must stay out of
            # the cell's output.
            with outer_tracer.start_as_current_span("ambient_sibling"):
                pass

            html, exc = run_cell_with_trace(
                ipython_shell,
                "ommx_isolation_sentinel = 1",
            )

        assert exc is None
        assert ipython_shell.user_ns.pop("ommx_isolation_sentinel") == 1
        # The cell got a fresh trace, so neither ``ambient_sibling``
        # nor the still-open ``ambient_parent`` leak into the rendered
        # tree.
        assert "ambient_sibling" not in html
        assert "ambient_parent" not in html
        assert "ommx_trace_cell" in html
        # Defensive: spans produced on the ambient trace are still
        # captured by the ambient-trace collector slot — they didn't
        # silently migrate into the cell's slot.
        ambient_spans = cell_collector.end_capture(ambient_trace_id)
        assert "ambient_sibling" in {s.name for s in ambient_spans}
    finally:
        _setup.reset_for_testing()


def test_run_cell_with_trace_supports_top_level_await(ipython_shell):
    """Using ``shell.run_cell`` (rather than ``shell.ex``) preserves the
    full IPython cell pipeline, including top-level ``await`` — so
    ``%%ommx_trace`` stays observational for async cells."""
    _setup.reset_for_testing()
    try:
        ipython_shell.user_ns.pop("ommx_await_sentinel", None)
        html, exc = run_cell_with_trace(
            ipython_shell,
            "import asyncio\n"
            "async def _probe():\n"
            "    return 99\n"
            "ommx_await_sentinel = await _probe()\n",
        )
        assert exc is None, exc
        assert ipython_shell.user_ns["ommx_await_sentinel"] == 99
        assert "ommx_trace_cell" in html
    finally:
        _setup.reset_for_testing()
