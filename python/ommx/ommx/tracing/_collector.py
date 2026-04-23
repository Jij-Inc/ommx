"""Collect finished OTel spans keyed by ``trace_id``.

``%%ommx_trace`` creates a root span for each cell and asks this collector
for every span that shares that cell's ``trace_id``. The cell magic is
the only expected caller — this module does not define a public API.
"""

from __future__ import annotations

import threading
from typing import Dict, List

from opentelemetry.sdk.trace import ReadableSpan, SpanProcessor


class _CellSpanCollector(SpanProcessor):
    """``SpanProcessor`` that stashes finished spans by ``trace_id``.

    The collector is registered once per ``TracerProvider`` (see
    :func:`._setup.ensure_collector_installed`) and lives for the duration
    of the notebook session. The cell magic retrieves and then discards
    the entries for its own ``trace_id`` to keep memory bounded.
    """

    def __init__(self) -> None:
        self._lock = threading.Lock()
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
            self._spans_by_trace.setdefault(ctx.trace_id, []).append(span)

    def shutdown(self) -> None:  # type: ignore[override]
        with self._lock:
            self._spans_by_trace.clear()

    def force_flush(self, timeout_millis: int = 30_000) -> bool:  # type: ignore[override]
        return True

    # ---- Cell-magic facing ------------------------------------------------

    def pop_trace(self, trace_id: int) -> List[ReadableSpan]:
        """Return (and remove) spans collected for ``trace_id``.

        Entries are removed on retrieval so the collector does not
        accumulate state across cells.
        """
        with self._lock:
            return self._spans_by_trace.pop(trace_id, [])
