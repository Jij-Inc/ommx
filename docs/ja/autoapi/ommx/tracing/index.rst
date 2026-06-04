ommx.tracing
============

.. py:module:: ommx.tracing

.. autoapi-nested-parse::

   Per-cell / per-block tracing for OMMX.

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

       from ommx.tracing import capture_trace, render_text_tree, save_chrome_trace, traced

       with capture_trace() as trace:
           solution = instance.evaluate(state)
       print(render_text_tree(trace))
       save_chrome_trace(trace, "trace.json")

       @traced(output="process.json")
       def process():
           ...

   The public surface is intentionally small:

   * :class:`capture_trace` — context manager; ``__enter__`` returns a
     :class:`TraceResult` placeholder that ``__exit__`` fills in (for
     success *and* for exceptions — information is never dropped).
   * :class:`TraceResult` — completed trace data: ``request``, ``spans``,
     ``otlp_protobuf()``.
   * :func:`render_text_tree`, :func:`chrome_trace_json`,
     :func:`save_chrome_trace` — render or save a :class:`TraceResult`.
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
   ``_capture``, ``_decorator``, ``_result``) is internal and may change
   without notice. Reach for them only if you are building on top of this
   module and can tolerate breakage.



Classes
-------

.. autoapisummary::

   ommx.tracing.TraceResult
   ommx.tracing.capture_trace


Functions
---------

.. autoapisummary::

   ommx.tracing.chrome_trace_json
   ommx.tracing.load_ipython_extension
   ommx.tracing.render_text_tree
   ommx.tracing.save_chrome_trace
   ommx.tracing.traced
   ommx.tracing.unload_ipython_extension


Package Contents
----------------

.. py:class:: TraceResult

   Populated result of a ``capture_trace`` block.

   Filled in by :class:`capture_trace` on ``__exit__`` (including the
   exception path, so the caller can always inspect the trace even
   when the block raised).


   .. py:method:: from_otlp_protobuf(payload: bytes) -> TraceResult
      :classmethod:


      Build a trace result from an OMMX trace payload.



   .. py:method:: otlp_protobuf() -> bytes

      Return OTLP protobuf bytes stored in Experiment traces.



   .. py:attribute:: request
      :type:  opentelemetry.proto.collector.trace.v1.trace_service_pb2.ExportTraceServiceRequest


   .. py:property:: spans
      :type: list[opentelemetry.proto.trace.v1.trace_pb2.Span]


      Flattened OTLP protobuf spans exported in this trace result.



.. py:class:: capture_trace(name: str = _DEFAULT_ROOT_SPAN_NAME, tracer_name: str = _TRACER_NAME)

   Context manager that captures every OTel span inside the block.

   The root span is started with an explicit empty OTel ``Context`` so
   each block gets its own fresh ``trace_id`` regardless of any
   ambient spans — the collector keys captures by ``trace_id``, so
   without this guard sibling spans from unrelated instrumentation
   would bleed into the result.


.. py:function:: chrome_trace_json(result: ommx.tracing._result.TraceResult) -> str

.. py:function:: load_ipython_extension(ipython: IPython.core.interactiveshell.InteractiveShell) -> None

   Register the ``%%ommx_trace`` cell magic on ``ipython``.

   Invoked by IPython when the user runs ``%load_ext ommx.tracing``.
   Safe to call more than once — later calls are no-ops since IPython
   dedupes magics by name.


.. py:function:: render_text_tree(result: ommx.tracing._result.TraceResult) -> str

   Render ``result`` as a nested ASCII tree, one root per top-level span.

   The tree preserves parent→child relationships as recorded by OTel.
   Siblings are sorted by start time so the output reflects execution
   order.


.. py:function:: save_chrome_trace(result: ommx.tracing._result.TraceResult, path: Union[str, pathlib.Path]) -> None

   Write ``result`` as Chrome Trace JSON to ``path``.

   Overwrites any existing file. The UTF-8 encoding matches the JSON
   spec and is what Perfetto / speedscope / ``chrome://tracing`` all accept.


.. py:function:: traced(func: _F) -> _F
                 traced(*, name: Optional[str] = ..., output: Optional[Union[str, pathlib.Path]] = ...) -> Callable[[_F], _F]

   Decorator that runs the wrapped function under :class:`capture_trace`.

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


.. py:function:: unload_ipython_extension(ipython: IPython.core.interactiveshell.InteractiveShell) -> None

   IPython extension hook, kept as a no-op.

   Removing a previously-registered magic leaves the shell in an
   awkward state (the name still resolves for tab completion), and
   there is no user-observable state to tear down — the collector
   sheds its entries on retrieval already.


