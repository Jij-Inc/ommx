"""``%%ommx_trace`` cell magic implementation.

Registered via ``%load_ext ommx.tracing``. The magic is a thin wrapper
around :class:`._capture.capture_trace`: it opens a ``capture_trace``
block named ``ommx_trace_cell``, executes the cell body through
``InteractiveShell.run_cell`` so the full IPython pipeline (input
transforms, top-level ``await``, line magics) applies, and renders
the collected trace as HTML.

The only cell-magic-specific extras are:

* We suppress ``run_cell``'s inbuilt ``showtraceback`` for the inner
  call so the *outer* ``run_cell`` (which invoked the magic) prints
  the traceback exactly once after our re-raise.
* We re-raise the cell's exception so notebook automation
  (``nbconvert --execute``, papermill, pytest-nbval, …) still sees
  traced cells fail when the user code raised.

Everything else (fresh trace context, active-trace collector gating,
HTML rendering with the ``[ERROR]`` marker) comes from
``capture_trace`` / ``_render``.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Optional, Tuple

from ._capture import capture_trace
from ._render import render_cell_output_html


if TYPE_CHECKING:  # pragma: no cover - type hints only
    from IPython.core.interactiveshell import InteractiveShell


_CELL_ROOT_SPAN_NAME = "ommx_trace_cell"


def run_cell_with_trace(
    shell: "InteractiveShell",
    cell: str,
) -> Tuple[str, Optional[BaseException]]:
    """Execute ``cell`` inside an ``ommx_trace_cell`` root span.

    Returns ``(html_blob, error)`` where ``html_blob`` is the rendered
    trace HTML (always produced, even if the cell raised) and
    ``error`` is the exception raised during execution or ``None``.
    Splitting rendering from re-raise keeps the function testable
    without driving IPython's display machinery; the magic entry
    point (:func:`register_magic`) is what actually re-raises.
    """
    cell_exc: Optional[BaseException] = None
    with capture_trace(_CELL_ROOT_SPAN_NAME) as trace_result:
        # Suppress the inner ``showtraceback``: ``run_cell`` calls it
        # unconditionally on error, but the outer ``run_cell`` will
        # also call it once our re-raise reaches it, and we don't
        # want the traceback printed twice.
        original_showtraceback = shell.showtraceback
        shell.showtraceback = lambda *args, **kwargs: None
        try:
            result = shell.run_cell(cell, store_history=False)
        finally:
            shell.showtraceback = original_showtraceback

        # Surface both parse-time and runtime errors.
        # ``error_before_exec`` is typed as ``BaseException | True |
        # None`` in IPython's stubs (``True`` was a legacy sentinel),
        # so narrow through ``isinstance`` first.
        err_before = result.error_before_exec
        cell_exc = (
            err_before
            if isinstance(err_before, BaseException)
            else result.error_in_exec
        )

        if cell_exc is not None:
            # ``shell.run_cell`` caught the exception internally rather
            # than letting it propagate, so the ``capture_trace`` block
            # is about to exit *normally* and the root span would
            # close with an OK status — losing the ``[ERROR]`` marker
            # in the rendered tree even though the cell did fail.
            # Explicitly mark the root as failed and record the
            # exception as a span event.
            from opentelemetry import trace as otel_trace
            from opentelemetry.trace.status import Status, StatusCode

            root = otel_trace.get_current_span()
            root.set_status(Status(StatusCode.ERROR, str(cell_exc)))
            root.record_exception(cell_exc)

    return render_cell_output_html(trace_result.spans), cell_exc


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
