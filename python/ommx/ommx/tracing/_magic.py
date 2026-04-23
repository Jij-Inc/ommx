"""``%%ommx_trace`` cell magic implementation.

Registered via ``%load_ext ommx.tracing``. The magic wraps cell execution
in a single OTel root span, asks :class:`._collector._CellSpanCollector`
to stash only that trace's spans, and displays the result as HTML.

Cell-body execution goes through :meth:`InteractiveShell.ex` rather
than :meth:`InteractiveShell.run_cell`: we want the exception raised by
the user's cell to propagate naturally so the outer (magic-invoking)
``run_cell`` reports the failure to upstream automation
(``nbconvert --execute``, papermill, pytest-nbval, …). ``run_cell``
captures and displays the exception itself, which would either swallow
it or double-display the traceback once we re-raise.
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
    is the exception raised during execution or ``None``. Splitting
    rendering from re-raise keeps the function testable without having
    to drive IPython's display machinery; the cell magic entry point
    (:func:`register_magic`) is what actually re-raises.

    ``end_capture`` sits in a ``finally`` so the collector's active-trace
    set does not leak the trace_id on ``KeyboardInterrupt`` or
    ``SystemExit``.
    """
    collector = ensure_collector_installed()
    tracer = trace.get_tracer(_CELL_TRACER_NAME)

    cell_exc: Optional[BaseException] = None
    trace_id: Optional[int] = None
    try:
        with tracer.start_as_current_span(_CELL_ROOT_SPAN_NAME) as root:
            trace_id = root.get_span_context().trace_id
            collector.begin_capture(trace_id)
            try:
                # ``shell.ex`` exec's the string in the user namespace
                # with minimal IPython machinery; exceptions propagate
                # naturally and are caught one layer up.
                shell.ex(cell)
            except BaseException as exc:  # noqa: BLE001 - re-raised by caller
                cell_exc = exc
    finally:
        # Call ``end_capture`` *after* the ``with`` block exits so the
        # root span's ``on_end`` fires first and lands in the collector.
        # Moving this into the inner ``finally`` (inside ``with``) would
        # drop the root span itself from the rendered tree. Using an
        # outer ``finally`` also makes sure the collector's active-trace
        # set is cleared even on ``KeyboardInterrupt`` or ``SystemExit``.
        spans = collector.end_capture(trace_id) if trace_id is not None else []

    return render_cell_output_html(spans), cell_exc


def register_magic(shell: "InteractiveShell") -> None:
    """Attach ``%%ommx_trace`` to ``shell``. Called from ``load_ipython_extension``."""
    from IPython.core.magic import register_cell_magic
    from IPython.display import HTML, display

    @register_cell_magic("ommx_trace")  # type: ignore[misc]
    def _ommx_trace(line: str, cell: str) -> None:
        # ``line`` is unused in the MVP; reserved for future flags
        # (e.g. a filename override for the download link).
        del line
        html_blob, cell_exc = run_cell_with_trace(shell, cell)
        # Display the trace first so the user sees it above any
        # traceback that IPython will print when we re-raise.
        display(HTML(html_blob))
        if cell_exc is not None:
            # Propagate so the outer ``run_cell`` records the failure.
            # Without this, notebook automation (``nbconvert --execute``,
            # papermill, pytest-nbval) would silently treat traced cells
            # as successful even when the user code raised.
            #
            # Python preserves the original traceback on ``cell_exc``
            # via ``__traceback__``, so IPython's display will still
            # show the frames inside the user's cell.
            raise cell_exc

    # Reference for static analysers; decorator already installed the magic.
    del _ommx_trace
