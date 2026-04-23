"""Render a collected trace as a text tree and a Chrome Trace JSON blob.

Both renderers take a ``list[ReadableSpan]`` and are independent of the
cell-magic plumbing — :func:`render_text_tree` is useful from a plain REPL
as well, and the JSON writer is what backs the download link surfaced by
the magic's HTML output.
"""

from __future__ import annotations

import base64
import html
import json
from typing import Dict, Iterable, List, Optional, Sequence, Set

from opentelemetry.sdk.trace import ReadableSpan
from opentelemetry.trace.status import StatusCode


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _duration_ms(span: ReadableSpan) -> float:
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


def _status_marker(span: ReadableSpan) -> str:
    """Return ``" [ERROR]"`` when the span recorded a failure, else ``""``.

    OTel sets ``Status(ERROR)`` on spans whose context manager saw an
    exception (``start_as_current_span`` defaults to
    ``record_exception=True``). Surfacing that in the tree makes it
    obvious which leaf failed when the user re-reads a trace for a
    crashed block.
    """
    status = getattr(span, "status", None)
    if status is not None and status.status_code == StatusCode.ERROR:
        return " [ERROR]"
    return ""


def _interesting_attributes(span: ReadableSpan) -> str:
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


def render_text_tree(spans: Sequence[ReadableSpan]) -> str:
    """Render ``spans`` as a nested ASCII tree, one root per top-level span.

    The tree preserves parent→child relationships as recorded by OTel.
    Siblings are sorted by start time so the output reflects execution
    order.
    """
    if not spans:
        return "(no spans)"

    span_ids: Set[int] = set()
    children: Dict[Optional[int], List[ReadableSpan]] = {}
    for span in spans:
        ctx = span.context
        if ctx is None:
            continue
        span_ids.add(ctx.span_id)
        parent_id = span.parent.span_id if span.parent is not None else None
        children.setdefault(parent_id, []).append(span)

    # A span's parent may not be in `spans` (e.g. the cell root was created
    # outside the collected set). Treat those as roots too so we never drop
    # branches on the floor.
    roots: List[ReadableSpan] = []
    for parent_id, kids in children.items():
        if parent_id is None or parent_id not in span_ids:
            roots.extend(kids)
    roots.sort(key=lambda s: s.start_time or 0)

    lines: List[str] = []

    def walk(span: ReadableSpan, prefix: str, is_last: bool) -> None:
        marker = "└── " if is_last else "├── "
        lines.append(
            f"{prefix}{marker}{span.name} "
            f"({_format_duration(_duration_ms(span))})"
            f"{_status_marker(span)}"
            f"{_interesting_attributes(span)}"
        )
        ctx = span.context
        if ctx is None:
            return
        kids = children.get(ctx.span_id, [])
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


def to_chrome_trace(spans: Iterable[ReadableSpan]) -> dict:
    """Convert a list of OTel spans to the Chrome Trace Event Format.

    Uses complete-duration events (``ph: "X"``) with ``ts``/``dur`` in
    microseconds, which is what Perfetto, speedscope, and
    ``chrome://tracing`` all consume. The per-span ``args`` dict carries
    OTel attributes so they show up in tool tooltips.
    """
    events: List[dict] = []
    for span in spans:
        if span.start_time is None or span.end_time is None:
            continue
        ts_us = span.start_time // 1_000
        dur_us = max((span.end_time - span.start_time) // 1_000, 1)
        args = {k: _attribute_to_json(v) for k, v in (span.attributes or {}).items()}
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


def chrome_trace_json(spans: Iterable[ReadableSpan]) -> str:
    return json.dumps(to_chrome_trace(spans))


# ---------------------------------------------------------------------------
# HTML glue for the cell magic
# ---------------------------------------------------------------------------


def render_cell_output_html(
    spans: Sequence[ReadableSpan],
    *,
    download_filename: str = "ommx_trace.json",
) -> str:
    """HTML blob for ``display(HTML(...))`` from :mod:`_magic`.

    Renders the text tree inside a ``<pre>`` and attaches a download link
    pointing at a base64 data URL of the Chrome Trace JSON. This keeps
    the magic dependency-free — no ipywidgets, no assets.
    """
    tree = html.escape(render_text_tree(spans))
    payload = chrome_trace_json(spans)
    b64 = base64.b64encode(payload.encode("utf-8")).decode("ascii")
    data_url = f"data:application/json;base64,{b64}"
    size_kb = len(payload) / 1024
    # ``quote=True`` escapes both ``"`` and ``'`` — essential when the
    # value lands inside an HTML attribute where an un-escaped quote
    # would terminate the attribute and allow injection. Cell magic
    # callers currently pass a literal default, but the parameter is
    # public, so harden it anyway.
    safe_filename = html.escape(download_filename, quote=True)
    return (
        '<div class="ommx-trace">'
        f"<pre>{tree}</pre>"
        f'<p><a href="{data_url}" download="{safe_filename}">'
        f"Download Chrome Trace JSON ({size_kb:.1f} KB)"
        "</a> — open in Perfetto, speedscope, or <code>chrome://tracing</code>.</p>"
        "</div>"
    )
