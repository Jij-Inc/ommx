"""Script-side tracing API: ``capture_trace`` and ``@traced``.

The Jupyter cell magic (``%%ommx_trace``) is great inside a notebook
but useless from a plain Python script. This module exposes the same
``_collector`` / ``_render`` machinery through a context manager and a
decorator so callers outside IPython get the same tree + JSON output.

Usage::

    from ommx.tracing import capture_trace

    with capture_trace() as trace:
        instance = Instance.from_bytes(blob)
        solution = instance.evaluate(state)

    print(trace.text_tree())
    trace.save_chrome_trace("out.json")

Or as a decorator::

    from ommx.tracing import traced

    @traced(output="process.json")
    def process():
        ...

    process()  # writes process.json on return *and* on exception

**Exception handling.** Everything inside the managed block still
raises normally — we never swallow. Before the exception propagates:

1. OTel's ``start_as_current_span`` records the exception on the root
   span (``record_exception=True`` + ``set_status(ERROR)``), so any
   exporter downstream (Chrome Trace consumers, OTLP backends) sees
   the failure.
2. ``TraceResult.spans`` is populated from the collector, so the
   caller can still inspect or save the trace from an outer ``try``
   block's ``except`` / ``finally`` arm.
3. :func:`._render.render_text_tree` flags spans with ``[ERROR]`` so
   the failure location is obvious in the rendered tree.
"""

from __future__ import annotations

import functools
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, List, Optional, Union, overload

from opentelemetry import context as otel_context
from opentelemetry import trace
from opentelemetry.sdk.trace import ReadableSpan

from ._render import chrome_trace_json, render_text_tree
from ._setup import ensure_collector_installed


# One tracer name for every entry point in ``ommx.tracing`` (the cell
# magic, the context manager, and the decorator). The name is a
# coarse instrumentation-scope label — the actual span *name* passed
# to ``tracer.start_as_current_span`` is what appears in the tree, so
# we don't need a per-entry-point tracer.
_TRACER_NAME = "ommx.tracing"
_DEFAULT_ROOT_SPAN_NAME = "ommx_trace_block"


@dataclass
class TraceResult:
    """Populated result of a ``capture_trace`` block.

    Filled in by :class:`capture_trace` on ``__exit__`` (including the
    exception path, so the caller can always inspect the trace even
    when the block raised).
    """

    spans: List[ReadableSpan] = field(default_factory=list)

    def text_tree(self) -> str:
        """Return the nested text tree — same renderer the cell magic uses."""
        return render_text_tree(self.spans)

    def chrome_trace_json(self) -> str:
        """Return a Chrome Trace Event Format JSON string."""
        return chrome_trace_json(self.spans)

    def save_chrome_trace(self, path: Union[str, Path]) -> None:
        """Write the Chrome Trace JSON to ``path`` (creating parents as needed).

        Overwrites any existing file. The UTF-8 encoding matches the
        JSON spec and is what Perfetto / speedscope /
        ``chrome://tracing`` all accept.
        """
        p = Path(path)
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(self.chrome_trace_json(), encoding="utf-8")


class capture_trace:  # noqa: N801 - context-manager factory, lowercase on purpose
    """Context manager that captures every OTel span inside the block.

    The root span is started with an explicit empty OTel ``Context`` so
    each block gets its own fresh ``trace_id`` regardless of any
    ambient spans — the collector keys captures by ``trace_id``, so
    without this guard sibling spans from unrelated instrumentation
    would bleed into the result.
    """

    def __init__(self, name: str = _DEFAULT_ROOT_SPAN_NAME) -> None:
        self._name = name
        self._result = TraceResult()
        self._trace_id: Optional[int] = None
        self._collector = None  # type: Any
        self._span_cm = None  # type: Any

    def __enter__(self) -> TraceResult:
        self._collector = ensure_collector_installed()
        tracer = trace.get_tracer(_TRACER_NAME)
        # ``context=Context()`` detaches from the ambient context so
        # the block's trace_id is always fresh — keeps the capture
        # window from pulling in unrelated sibling spans.
        self._span_cm = tracer.start_as_current_span(
            self._name,
            context=otel_context.Context(),
        )
        root = self._span_cm.__enter__()
        self._trace_id = root.get_span_context().trace_id
        self._collector.begin_capture(self._trace_id)
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
                self._result.spans = self._collector.end_capture(self._trace_id)
        # Never swallow the exception.
        return False


# ---------------------------------------------------------------------------
# @traced decorator
# ---------------------------------------------------------------------------


_F = Callable[..., Any]


@overload
def traced(func: _F) -> _F: ...


@overload
def traced(
    *,
    name: Optional[str] = ...,
    output: Optional[Union[str, Path]] = ...,
) -> Callable[[_F], _F]: ...


def traced(
    func: Optional[_F] = None,
    *,
    name: Optional[str] = None,
    output: Optional[Union[str, Path]] = None,
) -> Any:
    """Decorator that runs the wrapped function under :class:`capture_trace`.

    Supports all three call shapes::

        @traced
        def process(): ...

        @traced()
        def process(): ...

        @traced(name="build_qubo", output="qubo.json")
        def process(): ...

    If ``output`` is given, the Chrome Trace JSON is written to that
    path when the function returns **or raises** — information is
    never dropped. The exception, if any, is re-raised unchanged after
    the file is written.

    If ``name`` is omitted, the span is named after the function
    (``fn.__qualname__``) so traces from multiple decorated functions
    are easy to tell apart in the rendered tree.
    """

    def _decorator(fn: _F) -> _F:
        span_name = name if name is not None else fn.__qualname__

        @functools.wraps(fn)
        def _wrapper(*args, **kwargs):
            capture = capture_trace(span_name)
            result: Optional[TraceResult] = None
            try:
                with capture as r:
                    result = r
                    return fn(*args, **kwargs)
            finally:
                # ``capture.__exit__`` has populated ``result.spans``
                # by the time this ``finally`` fires (the ``with`` exit
                # runs before the enclosing try/finally's ``finally``).
                # Writing the trace here covers both the success and
                # the exception path, so the file is never missing
                # when the user expected one.
                if output is not None and result is not None:
                    result.save_chrome_trace(output)

        return _wrapper

    if func is not None:
        # ``@traced`` without parens.
        return _decorator(func)
    return _decorator
