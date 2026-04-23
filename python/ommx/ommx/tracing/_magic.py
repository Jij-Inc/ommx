"""``%%ommx_trace`` cell magic implementation.

Registered via ``%load_ext ommx.tracing``. The magic wraps cell execution
in a single OTel root span, asks :class:`._collector._CellSpanCollector`
to stash only that trace's spans, and displays the result as HTML.

Cell-body execution goes through :meth:`InteractiveShell.run_cell` so
the full IPython pipeline — input transforms, top-level ``await``, line
magics, cell-input cleanups — behaves the way it would in an untraced
cell. The point is that ``%%ommx_trace`` should be observational: a
user removes the magic, the cell body still does the same thing.

Re-raising without a double traceback is slightly delicate. ``run_cell``
catches exceptions internally and calls ``shell.showtraceback()`` before
returning; if we then re-raise the captured exception, the *outer*
``run_cell`` (the one that invoked our magic) prints it a second time.
We work around that by temporarily pointing ``shell.showtraceback`` at
a no-op during the inner call, so the traceback is shown exactly once —
by the outer ``run_cell`` when our re-raise reaches it.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Optional, Tuple

from opentelemetry import context as otel_context
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
    """Execute ``cell`` inside a fresh ``ommx_trace_cell`` root span.

    Returns ``(html_blob, error)`` where ``html_blob`` is the rendered
    trace output (always produced, even if the cell raised) and
    ``error`` is the exception raised during execution or ``None``.
    Splitting rendering from re-raise keeps the function testable
    without driving IPython's display machinery; the magic entry point
    (:func:`register_magic`) is what actually re-raises.

    The root span is started with an explicit empty OTel ``Context`` so
    the cell's ``trace_id`` is always fresh, detached from any ambient
    span that might be in scope. Without that detach, an ambient span
    installed by an unrelated extension would share a ``trace_id`` with
    the cell, and the collector — which keys captures by ``trace_id`` —
    would pull unrelated spans into the cell's output.
    """
    collector = ensure_collector_installed()
    tracer = trace.get_tracer(_CELL_TRACER_NAME)

    cell_exc: Optional[BaseException] = None
    trace_id: Optional[int] = None
    try:
        with tracer.start_as_current_span(
            _CELL_ROOT_SPAN_NAME,
            # ``context=Context()`` detaches from whatever context is
            # currently active, forcing a brand-new trace. Without this
            # the cell would inherit an ambient span's trace_id and the
            # collector's capture window would pull in unrelated spans.
            context=otel_context.Context(),
        ) as root:
            trace_id = root.get_span_context().trace_id
            collector.begin_capture(trace_id)

            # Suppress the inner ``showtraceback``: ``run_cell`` calls
            # it unconditionally on error, but the outer ``run_cell``
            # will also call it once our re-raise reaches it, and we
            # don't want the traceback printed twice.
            original_showtraceback = shell.showtraceback
            shell.showtraceback = lambda *args, **kwargs: None
            try:
                result = shell.run_cell(cell, store_history=False)
            finally:
                shell.showtraceback = original_showtraceback
            # Both parse-time and runtime errors are surfaced.
            # ``error_before_exec`` is typed as ``BaseException | True |
            # None`` in IPython's stubs (``True`` was a legacy sentinel)
            # so narrow through ``isinstance`` before handing it back.
            err_before = result.error_before_exec
            cell_exc = (
                err_before
                if isinstance(err_before, BaseException)
                else result.error_in_exec
            )
    finally:
        # Call ``end_capture`` *after* the ``with`` block exits so the
        # root span's ``on_end`` fires first and lands in the collector.
        # Moving this into the inner ``finally`` (inside ``with``) would
        # drop the root span itself from the rendered tree. The outer
        # ``finally`` also makes sure the collector's active-trace set
        # is cleared on ``KeyboardInterrupt`` / ``SystemExit``.
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
