"""Collect finished OTel spans for active cell traces.

``%%ommx_trace`` brackets each cell with :meth:`begin_capture` and
:meth:`end_capture` calls on the collector. Spans whose ``trace_id`` is
not between a matching begin/end pair are dropped immediately — without
this gate, a long-lived notebook with other instrumentation would leak
memory as unrelated traces accumulated in :attr:`_spans_by_trace`.
"""

from __future__ import annotations

import threading
from typing import Dict, List, Set

from opentelemetry.sdk.trace import ReadableSpan, SpanProcessor


class _CellSpanCollector(SpanProcessor):
    """``SpanProcessor`` that stashes spans for explicitly captured traces."""

    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._active_traces: Set[int] = set()
        self._spans_by_trace: Dict[int, List[ReadableSpan]] = {}

    # ---- SpanProcessor API -------------------------------------------------

    def on_start(self, span, parent_context=None) -> None:  # type: ignore[override]
        # Collection happens on end; nothing to do when a span starts.
        pass

    def on_end(self, span: ReadableSpan) -> None:  # type: ignore[override]
        ctx = span.context
        if ctx is None:
            return
        with self._lock:
            if ctx.trace_id not in self._active_traces:
                # Span from unrelated instrumentation; dropping avoids
                # unbounded memory growth in long-lived notebooks.
                return
            self._spans_by_trace.setdefault(ctx.trace_id, []).append(span)

    def shutdown(self) -> None:  # type: ignore[override]
        with self._lock:
            self._active_traces.clear()
            self._spans_by_trace.clear()

    def force_flush(self, timeout_millis: int = 30_000) -> bool:  # type: ignore[override]
        return True

    # ---- Cell-magic facing ------------------------------------------------

    def begin_capture(self, trace_id: int) -> None:
        """Start collecting spans tagged with ``trace_id``.

        Must be paired with :meth:`end_capture`. Re-registering the same
        ``trace_id`` while it is still active is a no-op.
        """
        with self._lock:
            self._active_traces.add(trace_id)

    def end_capture(self, trace_id: int) -> List[ReadableSpan]:
        """Stop collecting ``trace_id`` and return the spans gathered so far.

        Also drops the trace_id from the active set so any late-arriving
        spans are discarded rather than retained.
        """
        with self._lock:
            self._active_traces.discard(trace_id)
            return self._spans_by_trace.pop(trace_id, [])
