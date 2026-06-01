"""Script-side trace capture context manager.

The Jupyter cell magic (``%%ommx_trace``) is great inside a notebook
but useless from a plain Python script. This module exposes the same
collector through a context manager so callers outside IPython can get
a :class:`TraceResult`.

Usage::

    from ommx.tracing import capture_trace, render_text_tree, save_chrome_trace

    with capture_trace() as trace:
        instance = Instance.from_bytes(blob)
        solution = instance.evaluate(state)

    print(render_text_tree(trace))
    save_chrome_trace(trace, "out.json")

**Exception handling.** Everything inside the managed block still
raises normally — we never swallow. Before the exception propagates:

1. OTel's ``start_as_current_span`` records the exception on the root
   span (``record_exception=True`` + ``set_status(ERROR)``), so any
   exporter downstream (Chrome Trace consumers, OTLP backends) sees
   the failure.
2. ``TraceResult.request`` is populated from the collector, so the
   caller can still inspect or save the exported trace from an outer
   ``try`` block's ``except`` / ``finally`` arm.
3. :func:`._render.render_text_tree` flags spans with ``[ERROR]`` so
   the failure location is obvious in the rendered tree.
"""

from __future__ import annotations

from contextlib import AbstractContextManager
from typing import Optional

from opentelemetry import context as otel_context
from opentelemetry import trace
from opentelemetry.trace import Span

from ._collector import _TraceSpanCollector
from ._otlp import spans_to_otlp_request
from ._result import TraceResult
from ._setup import ensure_collector_installed


# One tracer name for every entry point in ``ommx.tracing`` (the cell
# magic, the context manager, and the decorator). The name is a
# coarse instrumentation-scope label — the actual span *name* passed
# to ``tracer.start_as_current_span`` is what appears in the tree, so
# we don't need a per-entry-point tracer.
_TRACER_NAME = "ommx.tracing"
_DEFAULT_ROOT_SPAN_NAME = "ommx_trace_block"


class capture_trace:  # noqa: N801 - context-manager factory, lowercase on purpose
    """Context manager that captures every OTel span inside the block.

    The root span is started with an explicit empty OTel ``Context`` so
    each block gets its own fresh ``trace_id`` regardless of any
    ambient spans — the collector keys captures by ``trace_id``, so
    without this guard sibling spans from unrelated instrumentation
    would bleed into the result.
    """

    def __init__(
        self,
        name: str = _DEFAULT_ROOT_SPAN_NAME,
        tracer_name: str = _TRACER_NAME,
    ) -> None:
        self._name = name
        self._tracer_name = tracer_name
        self._result = TraceResult()
        self._entered = False
        self._trace_id: Optional[int] = None
        self._collector: Optional[_TraceSpanCollector] = None
        self._span_cm: Optional[AbstractContextManager[Span]] = None

    def __enter__(self) -> TraceResult:
        if self._entered:
            raise RuntimeError("capture_trace context has already been entered")
        self._collector = ensure_collector_installed()
        tracer = trace.get_tracer(self._tracer_name)
        # ``context=Context()`` detaches from the ambient context so
        # the block's trace_id is always fresh — keeps the capture
        # window from pulling in unrelated sibling spans.
        self._span_cm = tracer.start_as_current_span(
            self._name,
            context=otel_context.Context(),
        )
        root = self._span_cm.__enter__()
        try:
            self._trace_id = root.get_span_context().trace_id
            self._collector.begin_capture(self._trace_id)
        except BaseException as exc:
            self._span_cm.__exit__(type(exc), exc, exc.__traceback__)
            self._span_cm = None
            self._trace_id = None
            raise
        self._entered = True
        return self._result

    def __exit__(self, exc_type, exc_val, exc_tb) -> bool:
        try:
            # Delegate to the span context manager so OTel records the
            # exception on the span (status=ERROR, exception event) —
            # ``record_exception=True`` is the default on
            # ``start_as_current_span``.
            if self._span_cm is not None:
                self._span_cm.__exit__(exc_type, exc_val, exc_tb)
        finally:
            # Populate the TraceResult whether or not the block raised,
            # so callers can inspect/save the trace from an outer
            # ``except`` or ``finally``.
            if self._collector is not None and self._trace_id is not None:
                spans = self._collector.end_capture(self._trace_id)
                self._result.request = spans_to_otlp_request(spans)
            self._span_cm = None
        # Never swallow the exception.
        return False
