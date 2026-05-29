"""Render an exported OTLP trace as a text tree or Chrome Trace JSON."""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Set

from opentelemetry.proto.collector.trace.v1.trace_service_pb2 import (
    ExportTraceServiceRequest,
)
from opentelemetry.proto.common.v1.common_pb2 import AnyValue
from opentelemetry.proto.trace.v1.trace_pb2 import Status as ProtoStatus
from opentelemetry.trace.status import StatusCode


@dataclass(frozen=True)
class _SpanView:
    name: str
    span_id: int
    parent_span_id: int | None
    start_time: int | None
    end_time: int | None
    status_code: StatusCode
    attributes: Mapping[str, Any] = field(default_factory=dict)


# ---------------------------------------------------------------------------
# Trace view
# ---------------------------------------------------------------------------


def _span_views(request: ExportTraceServiceRequest) -> list[_SpanView]:
    views: list[_SpanView] = []
    for resource_span in request.resource_spans:
        for scope_span in resource_span.scope_spans:
            for span in scope_span.spans:
                views.append(
                    _SpanView(
                        name=span.name,
                        span_id=_id_from_bytes(span.span_id),
                        parent_span_id=(
                            _id_from_bytes(span.parent_span_id)
                            if span.parent_span_id
                            else None
                        ),
                        start_time=_optional_int(span.start_time_unix_nano),
                        end_time=_optional_int(span.end_time_unix_nano),
                        status_code=_status_code_from_proto(span.status),
                        attributes=_attributes_from_proto(span.attributes),
                    )
                )
    return views


def _attributes_from_proto(attributes: Sequence[Any]) -> dict[str, Any]:
    return {
        str(attribute.key): _attribute_value_from_proto(attribute.value)
        for attribute in attributes
    }


def _attribute_value_from_proto(value: AnyValue) -> Any:
    field = value.WhichOneof("value")
    if field == "string_value":
        return value.string_value
    if field == "bool_value":
        return value.bool_value
    if field == "int_value":
        return value.int_value
    if field == "double_value":
        return value.double_value
    if field == "array_value":
        return [_attribute_value_from_proto(item) for item in value.array_value.values]
    if field == "kvlist_value":
        return _attributes_from_proto(value.kvlist_value.values)
    if field == "bytes_value":
        return bytes(value.bytes_value)
    return None


def _status_code_from_proto(status: ProtoStatus) -> StatusCode:
    if status.code == ProtoStatus.STATUS_CODE_ERROR:
        return StatusCode.ERROR
    if status.code == ProtoStatus.STATUS_CODE_OK:
        return StatusCode.OK
    return StatusCode.UNSET


def _id_from_bytes(value: bytes) -> int:
    return int.from_bytes(value, byteorder="big") if value else 0


def _optional_int(value: int) -> int | None:
    return value or None


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _duration_ms(span: _SpanView) -> float:
    """Return the span's duration in milliseconds.

    Open spans (which should not reach the renderer, but we still want
    to survive them) have ``end_time is None`` — report ``0.0`` instead
    of crashing.
    """
    if span.start_time is None or span.end_time is None:
        return 0.0
    return (span.end_time - span.start_time) / 1_000_000.0


def _format_duration(ms: float) -> str:
    if ms >= 1000:
        return f"{ms / 1000:.2f} s"
    if ms >= 1:
        return f"{ms:.2f} ms"
    return f"{ms * 1000:.1f} µs"


def _status_marker(span: _SpanView) -> str:
    """Return ``" [ERROR]"`` when the span recorded a failure, else ``""``.

    OTel sets ``Status(ERROR)`` on spans whose context manager saw an
    exception (``start_as_current_span`` defaults to
    ``record_exception=True``). Surfacing that in the tree makes it
    obvious which leaf failed when the user re-reads a trace for a
    crashed block.
    """
    if span.status_code == StatusCode.ERROR:
        return " [ERROR]"
    return ""


def _interesting_attributes(span: _SpanView) -> str:
    """Subset of attributes worth showing inline in the tree node.

    Filters out the ``tracing`` crate's bookkeeping keys (``busy_ns``,
    ``idle_ns``, ``thread.id``, ``code.*``) that are noise for human
    consumers. Everything else is fair game.
    """
    if not span.attributes:
        return ""
    skip = {"busy_ns", "idle_ns", "thread.id"}
    pairs = [
        f"{k}={v!r}"
        for k, v in span.attributes.items()
        if k not in skip and not k.startswith("code.")
    ]
    if not pairs:
        return ""
    return " [" + ", ".join(pairs) + "]"


# ---------------------------------------------------------------------------
# Text tree
# ---------------------------------------------------------------------------


def render_text_tree(request: ExportTraceServiceRequest) -> str:
    """Render ``request`` as a nested ASCII tree, one root per top-level span.

    The tree preserves parent→child relationships as recorded by OTel.
    Siblings are sorted by start time so the output reflects execution
    order.
    """
    spans = _span_views(request)
    if not spans:
        return "(no spans)"

    span_ids: Set[int] = set()
    children: Dict[Optional[int], List[_SpanView]] = {}
    for span in spans:
        span_ids.add(span.span_id)
        children.setdefault(span.parent_span_id, []).append(span)

    # A span's parent may not be in `spans` (e.g. the cell root was created
    # outside the collected set). Treat those as roots too so we never drop
    # branches on the floor.
    roots: List[_SpanView] = []
    for parent_id, kids in children.items():
        if parent_id is None or parent_id not in span_ids:
            roots.extend(kids)
    roots.sort(key=lambda s: s.start_time or 0)

    lines: List[str] = []

    def walk(span: _SpanView, prefix: str, is_last: bool) -> None:
        marker = "└── " if is_last else "├── "
        lines.append(
            f"{prefix}{marker}{span.name} "
            f"({_format_duration(_duration_ms(span))})"
            f"{_status_marker(span)}"
            f"{_interesting_attributes(span)}"
        )
        kids = children.get(span.span_id, [])
        kids.sort(key=lambda s: s.start_time or 0)
        next_prefix = prefix + ("    " if is_last else "│   ")
        for i, kid in enumerate(kids):
            walk(kid, next_prefix, i == len(kids) - 1)

    for i, root in enumerate(roots):
        walk(root, "", i == len(roots) - 1)

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Chrome Trace Event Format
# ---------------------------------------------------------------------------


def _attribute_to_json(value) -> object:
    """Coerce an OTel attribute value into something ``json.dumps`` accepts."""
    if isinstance(value, (str, int, float, bool)) or value is None:
        return value
    if isinstance(value, (list, tuple)):
        return [_attribute_to_json(v) for v in value]
    return str(value)


def to_chrome_trace(request: ExportTraceServiceRequest) -> dict:
    """Convert an exported OTLP trace request to Chrome Trace Event Format.

    Uses complete-duration events (``ph: "X"``) with ``ts``/``dur`` in
    microseconds, which is what Perfetto, speedscope, and
    ``chrome://tracing`` all consume. The per-span ``args`` dict carries
    OTel attributes so they show up in tool tooltips.
    """
    events: List[dict] = []
    for span in _span_views(request):
        if span.start_time is None or span.end_time is None:
            continue
        ts_us = span.start_time // 1_000
        dur_us = max((span.end_time - span.start_time) // 1_000, 1)
        attrs = span.attributes or {}
        args = {k: _attribute_to_json(v) for k, v in attrs.items()}
        # All events are placed on a single logical thread for the MVP
        # renderer. ``tracing``-crate spans carry a ``thread.id``
        # attribute; surfacing it as ``tid`` would let Perfetto /
        # speedscope lay out concurrent work on parallel tracks. Kept
        # out of scope until there's a workload that actually benefits.
        events.append(
            {
                "name": span.name,
                "cat": "ommx",
                "ph": "X",
                "ts": ts_us,
                "dur": dur_us,
                "pid": 1,
                "tid": 1,
                "args": args,
            }
        )
    events.sort(key=lambda e: (e["ts"], -e["dur"]))
    return {"traceEvents": events, "displayTimeUnit": "ms"}


def chrome_trace_json(request: ExportTraceServiceRequest) -> str:
    return json.dumps(to_chrome_trace(request))
