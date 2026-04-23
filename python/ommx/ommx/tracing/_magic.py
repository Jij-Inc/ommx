"""``%%ommx_trace`` cell magic implementation.

Registered via ``%load_ext ommx.tracing``. The magic wraps cell execution
in a single OTel root span, collects every span emitted during the cell
through :mod:`._collector`, and displays a text tree plus a Chrome Trace
JSON download link as the cell output.
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

    Returns ``(html_blob, exc)`` where ``html_blob`` is the rendered
    trace output (always produced, even if the cell raised) and ``exc``
    is the exception that escaped the cell, or ``None``. The caller is
    responsible for displaying the HTML and re-raising the exception —
    that split keeps this function testable without IPython's display
    machinery in scope.
    """
    collector = ensure_collector_installed()
    tracer = trace.get_tracer(_CELL_TRACER_NAME)

    cell_exc: Optional[BaseException] = None
    with tracer.start_as_current_span(_CELL_ROOT_SPAN_NAME) as root:
        trace_id = root.get_span_context().trace_id
        try:
            shell.ex(cell)
        except BaseException as exc:  # noqa: BLE001 - re-raised by caller
            cell_exc = exc

    spans = collector.pop_trace(trace_id)
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
        display(HTML(html_blob))
        if cell_exc is not None:
            raise cell_exc

    # Reference for static analysers; decorator already installed the magic.
    del _ommx_trace
