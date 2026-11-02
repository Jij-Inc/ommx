"""Inspect the time spent inside an OMMX operation.

A span records the start and end of an instrumented operation. Nested spans
show where time went within a parent operation. The parent includes its
children's time. A single root span tells you only the total duration.

**Capture one evaluation**

>>> from ommx import Instance
>>> from ommx.tracing import capture_trace, render_text_tree, save_chrome_trace
>>> problem = Instance.minimize()
>>> x = problem.new_binary("x")
>>> problem.objective = x
>>> with capture_trace("evaluate candidate") as trace:
...     solution = problem.evaluate({x.id: 1})
>>> assert solution.objective == 1
>>> assert "evaluate candidate" in render_text_tree(trace)

Inspect the tree in a terminal or display the result in Jupyter. To view a
timeline in Perfetto or another Chrome trace viewer, save the trace as JSON:

>>> from pathlib import Path
>>> from tempfile import TemporaryDirectory
>>> with TemporaryDirectory() as directory:
...     path = Path(directory) / "trace.json"
...     save_chrome_trace(trace, path)
...     assert path.stat().st_size > 0

To trace each call to a function, use :func:`traced`. The function still
returns its normal result. On failure, the decorator attempts to save the
trace and propagates the original exception.

>>> from ommx.tracing import traced
>>> with TemporaryDirectory() as directory:
...     path = Path(directory) / "evaluate.json"
...     @traced(output=path)
...     def evaluate(value):
...         return problem.evaluate({x.id: value})
...     assert evaluate(0).objective == 0 and path.is_file()

**Trace one notebook cell**

In a Jupyter kernel with the SDK installed, run this line in its own cell::

    %load_ext ommx.tracing

Then start the cell you want to measure with the magic::

    %%ommx_trace
    from ommx import Instance
    solution = Instance.minimize().evaluate({})

The output displays the span tree and a Chrome trace JSON link. Only
instrumented operations produce child spans. If the cell raises an exception,
the trace is displayed and the exception propagates.

For a solver breakdown, see :class:`ommx_highs_adapter.OMMXHighsAdapter`.
To keep a trace with an Experiment, see :attr:`ommx.experiment.SealedRun.trace`.
Local capture works without an external exporter. To send the same spans
to a service, see :doc:`OTLP export settings </operations/export_traces>`.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from ._capture import capture_trace
from ._decorator import traced
from ._render import chrome_trace_json, render_text_tree, save_chrome_trace
from ._result import TraceResult


if TYPE_CHECKING:  # pragma: no cover - typing only
    from IPython.core.interactiveshell import InteractiveShell


__all__ = [
    "TraceResult",
    "chrome_trace_json",
    "capture_trace",
    "load_ipython_extension",
    "render_text_tree",
    "save_chrome_trace",
    "traced",
    "unload_ipython_extension",
]


def load_ipython_extension(ipython: "InteractiveShell") -> None:
    """Register the ``%%ommx_trace`` cell magic on ``ipython``.

    Invoked by IPython when the user runs ``%load_ext ommx.tracing``.
    Safe to call more than once — later calls are no-ops since IPython
    dedupes magics by name.
    """
    from ._magic import register_magic

    register_magic(ipython)


def unload_ipython_extension(ipython: "InteractiveShell") -> None:
    """IPython extension hook, kept as a no-op.

    Removing a previously-registered magic leaves the shell in an
    awkward state (the name still resolves for tab completion), and
    there is no user-observable state to tear down — the collector
    sheds its entries on retrieval already.
    """
    del ipython
