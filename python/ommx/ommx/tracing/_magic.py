"""``%%ommx_trace`` cell magic implementation.

Registered via ``%load_ext ommx.tracing``. The magic wraps cell execution
in a single OTel root span, asks :class:`._collector._CellSpanCollector`
to stash only that trace's spans, and displays the result as HTML.

Execution goes through :meth:`InteractiveShell.run_cell` rather than
``shell.ex`` so that input transforms, display hooks, and error
formatting all behave the same way they would in a normal cell. The
cell-magic call site has already stored the raw cell in history, so we
pass ``store_history=False`` to avoid logging the body twice.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Optional, Tuple

from opentelemetry import trace

from ._render import render_cell_output_html
from ._setup import ensure_collector_installed


if TYPE_CHECKING:  # pragma: no cover - type hints only
    from IPython.core.interactiveshell import InteractiveShell


_CELL_TRACER_NAME = "ommx.tracing.cell"
_CELL_ROOT_SPAN_NAME = "ommx_trace_cell"


def run_cell_with_trace(
    shell: "InteractiveShell",
    cell: str,
) -> Tuple[str, Optional[BaseException]]:
    """Execute ``cell`` inside an ``ommx_trace_cell`` root span.

    Returns ``(html_blob, error)`` where ``html_blob`` is the rendered
    trace output (always produced, even if the cell raised) and ``error``
    is the exception raised during execution or ``None``. IPython's
    ``run_cell`` has already displayed the traceback when ``error`` is
    non-None; the caller uses it only for test assertions and does not
    need to re-raise.
    """
    collector = ensure_collector_installed()
    tracer = trace.get_tracer(_CELL_TRACER_NAME)

    with tracer.start_as_current_span(_CELL_ROOT_SPAN_NAME) as root:
        trace_id = root.get_span_context().trace_id
        collector.begin_capture(trace_id)
        try:
            # ``store_history=False`` avoids duplicating the cell body in
            # ``In[...]`` (the magic invocation itself is already stored).
            result = shell.run_cell(cell, store_history=False)
        finally:
            # ``end_capture`` must run even on SystemExit/KeyboardInterrupt
            # so the collector doesn't leak this trace's entries.
            pass

    spans = collector.end_capture(trace_id)
    # ``run_cell`` surfaces errors on the result object rather than
    # raising; IPython has already printed the traceback via its normal
    # display hooks by the time we get here.
    return render_cell_output_html(spans), result.error_in_exec


def register_magic(shell: "InteractiveShell") -> None:
    """Attach ``%%ommx_trace`` to ``shell``. Called from ``load_ipython_extension``."""
    from IPython.core.magic import register_cell_magic
    from IPython.display import HTML, display

    @register_cell_magic("ommx_trace")  # type: ignore[misc]
    def _ommx_trace(line: str, cell: str) -> None:
        # ``line`` is unused in the MVP; reserved for future flags
        # (e.g. a filename override for the download link).
        del line
        html_blob, _exc = run_cell_with_trace(shell, cell)
        # No re-raise: ``shell.run_cell`` already reported the error via
        # IPython's normal traceback UI, preserving the user-cell frames.
        display(HTML(html_blob))

    # Reference for static analysers; decorator already installed the magic.
    del _ommx_trace
