"""Per-cell / per-block tracing for OMMX.

Two user-facing APIs, sharing the same OTel collector + renderers:

**Jupyter cell magic** (best for notebooks)::

    %load_ext ommx.tracing

    %%ommx_trace
    instance = Instance.from_bytes(blob)
    solution = instance.evaluate(state)

Cell output shows a nested text tree of every span produced during the
cell (Rust and Python alike), annotated with durations and attributes,
plus a download link for the full trace in Chrome Trace Event Format.

**Context manager / decorator** (best for scripts, tests, CI)::

    from ommx.tracing import capture_trace, traced

    with capture_trace() as trace:
        solution = instance.evaluate(state)
    print(trace.text_tree())
    trace.save_chrome_trace("trace.json")

    @traced(output="process.json")
    def process():
        ...

The public surface is intentionally small:

* :class:`capture_trace` — context manager; ``__enter__`` returns a
  :class:`TraceResult` placeholder that ``__exit__`` fills in (for
  success *and* for exceptions — information is never dropped).
* :class:`TraceResult` — ``spans``, ``text_tree()``,
  ``chrome_trace_json()``, ``save_chrome_trace(path)``.
* :func:`traced` — decorator sugar on top of :class:`capture_trace`,
  optionally writing the Chrome Trace JSON to disk.
* :func:`load_ipython_extension` — wired by ``%load_ext ommx.tracing``;
  registers the ``%%ommx_trace`` cell magic. The collector itself is
  attached to the active OpenTelemetry ``TracerProvider`` lazily, on
  the first traced cell, so ``%load_ext`` stays cheap and cannot fail
  due to provider state that is still being configured further up the
  notebook.
* :func:`unload_ipython_extension` — pair for ``%unload_ext``. Kept as
  a no-op because IPython magics cannot be cleanly unregistered
  without disturbing user state; leaving the collector installed costs
  nothing.

Everything else (``_collector``, ``_render``, ``_setup``, ``_magic``,
``_capture``) is internal and may change without notice. Reach for
them only if you are building on top of this module and can tolerate
breakage.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from ._capture import TraceResult, capture_trace, traced


if TYPE_CHECKING:  # pragma: no cover - typing only
    from IPython.core.interactiveshell import InteractiveShell


__all__ = [
    "TraceResult",
    "capture_trace",
    "load_ipython_extension",
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
