"""Tests for rendering completed ``TraceResult`` values."""

from __future__ import annotations

import base64
import json
import urllib.parse

from opentelemetry.proto.trace.v1.trace_pb2 import Status as ProtoStatus

from ommx.tracing import (
    TraceResult,
    chrome_trace_json,
    render_text_tree,
    save_chrome_trace,
)


def _id(value: int) -> bytes:
    return value.to_bytes(8, byteorder="big")


def _add_span(
    result: TraceResult,
    name: str,
    span_id: int,
    *,
    parent_id: int | None = None,
    scope: str | None = None,
    start: int = 1_000_000_000,
    end: int | None = 1_001_000_000,
):
    scope_spans = result.request.resource_spans.add().scope_spans.add()
    if scope is not None:
        scope_spans.scope.name = scope
    span = scope_spans.spans.add()
    span.trace_id = (1).to_bytes(16, byteorder="big")
    span.span_id = _id(span_id)
    if parent_id is not None:
        span.parent_span_id = _id(parent_id)
    span.name = name
    span.start_time_unix_nano = start
    if end is not None:
        span.end_time_unix_nano = end
    return span


def test_render_text_tree_reflects_nesting():
    """Children appear indented under their parent, not as siblings."""
    result = TraceResult()
    _add_span(result, "outer", 1, start=1_000_000_000, end=1_010_000_000)
    _add_span(
        result,
        "inner",
        2,
        parent_id=1,
        start=1_001_000_000,
        end=1_002_000_000,
    )

    tree = render_text_tree(result)

    outer_line = next(line for line in tree.splitlines() if "outer" in line)
    inner_line = next(line for line in tree.splitlines() if "inner" in line)
    outer_indent = len(outer_line) - len(outer_line.lstrip())
    inner_indent = len(inner_line) - len(inner_line.lstrip())
    assert inner_indent > outer_indent


def test_render_text_tree_handles_empty():
    assert render_text_tree(TraceResult()) == "(no spans)"


def test_trace_result_repr_matches_text_tree():
    result = TraceResult()
    _add_span(result, "outer", 1, start=1_000_000_000, end=1_010_000_000)
    _add_span(result, "inner", 2, parent_id=1)

    assert repr(result) == render_text_tree(result)


def test_trace_result_repr_html_contains_download_link():
    result = TraceResult()
    _add_span(result, "outer", 1, start=1_000_000_000, end=1_010_000_000)
    _add_span(result, "inner", 2, parent_id=1)

    html = result._repr_html_()

    assert "<pre>" in html
    assert "outer" in html
    assert "inner" in html
    assert 'download="ommx_trace.json"' in html

    marker = 'href="data:application/json;base64,'
    start = html.index(marker) + len(marker)
    end = html.index('"', start)
    b64 = html[start:end]
    decoded = base64.b64decode(urllib.parse.unquote(b64)).decode("utf-8")
    parsed = json.loads(decoded)
    assert {event["name"] for event in parsed["traceEvents"]} == {"outer", "inner"}


def test_render_text_tree_hides_debug_source_attributes():
    result = TraceResult()
    span = _add_span(result, "work", 1, scope="ommx._rust")
    target = span.attributes.add()
    target.key = "target"
    target.value.string_value = "ommx::instance::evaluate"
    adapter = span.attributes.add()
    adapter.key = "adapter"
    adapter.value.string_value = "ommx_highs_adapter.adapter.OMMXHighsAdapter"

    tree = render_text_tree(result)

    assert "{scope=ommx._rust}" in tree
    assert "target=" not in tree
    assert "adapter='ommx_highs_adapter.adapter.OMMXHighsAdapter'" in tree


def test_render_text_tree_displays_instrumentation_scope():
    result = TraceResult()
    _add_span(result, "solve", 1, scope="ommx.adapter.highs")

    tree = render_text_tree(result)

    assert "solve" in tree
    assert "{scope=ommx.adapter.highs}" in tree


def test_render_text_tree_marks_error_spans():
    """Spans with ``Status(ERROR)`` are flagged in the text tree."""
    result = TraceResult()
    _add_span(result, "outer", 1)
    inner = _add_span(result, "inner", 2, parent_id=1)
    inner.status.code = ProtoStatus.STATUS_CODE_ERROR

    tree = render_text_tree(result)

    outer_line = next(line for line in tree.splitlines() if "outer" in line)
    inner_line = next(line for line in tree.splitlines() if "inner" in line)
    assert "[ERROR]" not in outer_line
    assert "[ERROR]" in inner_line


def test_render_text_tree_does_not_mark_successful_spans():
    result = TraceResult()
    _add_span(result, "clean", 1)
    assert "[ERROR]" not in render_text_tree(result)


def test_chrome_trace_json_is_valid_json_with_X_events():
    result = TraceResult()
    work = _add_span(result, "work", 1, scope="ommx.adapter.highs")
    attribute = work.attributes.add()
    attribute.key = "batch_size"
    attribute.value.int_value = 42

    parsed = json.loads(chrome_trace_json(result))

    assert parsed["displayTimeUnit"] == "ms"
    events = parsed["traceEvents"]
    assert {event["name"] for event in events} == {"work"}
    for event in events:
        assert event["ph"] == "X"
        assert isinstance(event["ts"], int) and event["ts"] > 0
        assert isinstance(event["dur"], int) and event["dur"] >= 1
        assert event["cat"] == "ommx.adapter.highs"
    assert events[0]["args"]["batch_size"] == 42
    assert events[0]["args"]["otel.scope.name"] == "ommx.adapter.highs"


def test_chrome_trace_json_skips_open_spans():
    """Spans without an end time are omitted from Chrome Trace output."""
    result = TraceResult()
    _add_span(result, "still_running", 1, end=None)

    parsed = json.loads(chrome_trace_json(result))

    assert parsed["traceEvents"] == []


def test_save_chrome_trace_writes_valid_json(tmp_path):
    result = TraceResult()
    _add_span(result, "work", 1)

    out = tmp_path / "nested" / "trace.json"
    save_chrome_trace(result, out)

    assert out.exists(), "save_chrome_trace should create parent dirs"
    parsed = json.loads(out.read_text(encoding="utf-8"))
    assert parsed["displayTimeUnit"] == "ms"
    assert parsed["traceEvents"]
