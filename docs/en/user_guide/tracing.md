---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# Tracing and Profiling

OMMX emits [OpenTelemetry](https://opentelemetry.io/) spans at selected entry points. Two thin wrappers in `ommx.tracing` turn that stream into something you can actually read:

- **`%%ommx_trace`** — a Jupyter cell magic that renders the spans produced during a single cell as a nested text tree, plus a download link for the full trace in Chrome Trace Event Format.
- **{class}`~ommx.tracing.capture_trace` / {func}`~ommx.tracing.traced`** — a context manager and decorator for the same workflow from plain Python scripts, tests, and CI.

Both entry points share one in-process collector. You do **not** need to install an OTel exporter or configure anything at import time: the collector installs itself lazily on first use. Ship the trace to a full OTel backend only when you need to — see [Using your own TracerProvider](#own-tracer-provider) below.

## Quick Tour

### Cell magic (`%%ommx_trace`)

Load the extension once per notebook (typically in the first cell):

```
%load_ext ommx.tracing
```

Then prefix any cell with `%%ommx_trace`:

```
%%ommx_trace
from ommx.v1 import Instance, DecisionVariable

x = DecisionVariable.binary(0, name="x")
y = DecisionVariable.binary(1, name="y")
instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints={},
    sense=Instance.MAXIMIZE,
)
solution = instance.evaluate({0: 1.0, 1: 1.0})
```

The cell output shows two things:

1. A nested **text tree** of every span produced in the cell, annotated with duration and the most useful span attributes.
2. A **download link** for the full trace in [Chrome Trace Event Format](https://docs.google.com/document/d/1CvAClvFfyA5R-PhYUmn5OOQtYMH4h6I0nSsKchNAySU/preview). Drop that JSON file into [Perfetto](https://ui.perfetto.dev/), [speedscope](https://www.speedscope.app/), or `chrome://tracing` to explore the trace as a flame graph.

```{note}
The rendered cell output (text tree + download link) is a minimal starting point and is expected to evolve — for example, an inline interactive flame graph is on the roadmap. Treat the exact layout and markup as unstable.
```

When the cell raises, the trace HTML is still rendered (with `[ERROR]` marking the failing span) *and* the exception is re-raised so notebook automation — `nbconvert --execute`, papermill, pytest-nbval — still sees the failure.

### Context manager (`capture_trace`)

The same machinery is available from plain Python:

```{code-cell} ipython3
from ommx.tracing import capture_trace
from ommx.v1 import Instance, DecisionVariable

x = DecisionVariable.binary(0, name="x")
y = DecisionVariable.binary(1, name="y")

instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints={},
    sense=Instance.MAXIMIZE,
)

with capture_trace() as trace:
    solution = instance.evaluate({0: 1.0, 1: 1.0})

print(trace.text_tree())
```

`trace` is a {class}`~ommx.tracing.TraceResult` populated when the block exits:

- {attr}`~ommx.tracing.TraceResult.spans` — the raw list of {class}`~opentelemetry.sdk.trace.ReadableSpan` for custom processing.
- {meth}`~ommx.tracing.TraceResult.text_tree` — the same nested renderer the cell magic uses.
- {meth}`~ommx.tracing.TraceResult.chrome_trace_json` — returns the trace as a JSON string.
- {meth}`~ommx.tracing.TraceResult.save_chrome_trace` — writes the JSON to disk (creates parent directories as needed).

If the block raises, `trace.spans` is still populated (with the failing span flagged as `[ERROR]`), so you can inspect or save it from an outer `except` or `finally`. The original exception propagates unchanged — OMMX never swallows.

```{code-cell} ipython3
import tempfile
from pathlib import Path

output_path = Path(tempfile.gettempdir()) / "ommx_trace.json"
trace.save_chrome_trace(output_path)
print(f"Wrote {output_path.stat().st_size} bytes to {output_path}")
```

### Decorator (`@traced`)

{func}`~ommx.tracing.traced` is sugar on top of {class}`~ommx.tracing.capture_trace`:

```{code-cell} ipython3
import tempfile
from pathlib import Path

from ommx.tracing import traced

evaluate_output = Path(tempfile.gettempdir()) / "evaluate_trace.json"

@traced(output=str(evaluate_output))
def evaluate_once(inst):
    return inst.evaluate({0: 1.0, 1: 1.0})

solution = evaluate_once(instance)
print(f"Wrote trace to {evaluate_output}")
```

All three call shapes are supported:

```python
@traced
def f(): ...

@traced()
def f(): ...

@traced(name="build_qubo", output="qubo.json")
def f(): ...
```

Key properties:

- If `name` is omitted, the root span name defaults to `fn.__qualname__`, so traces from multiple decorated functions are easy to tell apart.
- When `output` is set, the Chrome Trace JSON is written on normal return, and the decorator also **attempts** to write it on exception. On the exception path, save errors (e.g. I/O failures) are intentionally suppressed so they do not replace the original exception — so saving on exception is best-effort.
- `async def` is supported. The decorator detects coroutine functions with `inspect.iscoroutinefunction` and awaits them inside the trace block; without that detection, the capture window would close before the coroutine ran and every span would be dropped.

## Span Naming Convention

OMMX relies on `tracing`'s default span names — the bare function name (e.g. `evaluate`, `reduce_capabilities`, `push`, `pull`). The fully-qualified module path is carried by the OTel **instrumentation scope**, so you can still tell two `evaluate` spans from different modules apart by looking at the scope name rather than by munging the span name.

When the same method exists on multiple types (for example `Artifact::push` on `OciArchive` vs `Remote` storage backends), the Rust side disambiguates via span **fields** — e.g. `fields(artifact_storage = "oci_archive")` — rather than bespoke span names. Those fields show up as OTel attributes in the tree and in the Chrome Trace `args` dict.

(own-tracer-provider)=
## Using Your Own TracerProvider

`ommx.tracing` installs an in-process {class}`~opentelemetry.sdk.trace.TracerProvider` only if none is already registered. If you need spans to flow to an external backend (OTLP, Jaeger, Honeycomb, …), configure your provider **before the first call into the OMMX extension**:

```python
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter

provider = TracerProvider()
provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter()))
trace.set_tracer_provider(provider)

# Now import / call OMMX as usual. `%%ommx_trace` and `capture_trace`
# will attach their collector to your provider alongside the OTLP exporter.
from ommx.v1 import Instance
```

Two things to keep in mind:

1. **Configure your provider before the first call to `ommx.tracing` and before the first call into the Rust extension.** OpenTelemetry's Python API only honours the first {func}`~opentelemetry.trace.set_tracer_provider` call, and on first use `ommx.tracing` installs a default {class}`~opentelemetry.sdk.trace.TracerProvider` itself if nothing is set — after that point, a later `set_tracer_provider(your_provider)` is silently ignored. The Rust → Python tracing bridge is also initialized on the first instrumented Rust call, so configure OTel at the very top of your script / notebook.
2. **`ommx.tracing` attaches its collector to whichever provider is active** — it does not replace yours. Spans reach both the OMMX renderer and your OTLP exporter.

If you run with a non-SDK provider that does not support {meth}`~opentelemetry.sdk.trace.TracerProvider.add_span_processor` (rare, but some vendor SDKs do this), {class}`~ommx.tracing.capture_trace` raises a `RuntimeError` at `__enter__` with a pointer to the fix. Install an {class}`opentelemetry.sdk.trace.TracerProvider` yourself, and add your exporter as another `SpanProcessor` on the same provider.

## Troubleshooting

### I see `(no spans)` in the tree

Most commonly: the traced block didn't actually call into any instrumented OMMX code path. The collector captures spans whose `trace_id` falls inside the {class}`~ommx.tracing.capture_trace` window, and only instrumented call sites produce spans — raw Python control flow does not. Not every OMMX method is instrumented; constructors and simple accessors typically are not. Double-check that the block reaches an instrumented call (`Instance.evaluate`, `Instance.evaluate_samples`, `Instance.reduce_capabilities`, the `Artifact` `push` / `pull` / `load` / `save` entry points, adapter `solve`, etc.).

A second possibility: a non-SDK {class}`~opentelemetry.sdk.trace.TracerProvider` is active and `ommx.tracing` couldn't attach its collector. If that were the case, the first {class}`~ommx.tracing.capture_trace` call would have raised `RuntimeError` — see the message for the remediation.

### My OTLP backend shows the trace but the cell magic shows `(no spans)`

The collector is keyed on `trace_id`. {class}`~ommx.tracing.capture_trace` (and the cell magic) deliberately start with a **fresh** OTel context so the block gets a new `trace_id` — this is what keeps unrelated ambient spans from bleeding into the capture window. That also means spans you start yourself with {meth}`tracer.start_as_current_span(..., context=...) <opentelemetry.trace.Tracer.start_as_current_span>` from an unrelated parent won't show up in the cell-magic output, even though they do reach OTLP. Use the cell magic / {class}`~ommx.tracing.capture_trace` block as the outermost span, and nest your own spans inside it.

### Concurrency and async

Inside a {class}`~ommx.tracing.capture_trace` block, spans from the same logical thread nest correctly because OTel propagates the current span via a context variable. A few caveats:

- **Background threads** started *outside* the block do not inherit the block's OTel context. Spans from those threads won't be captured.
- **`asyncio` tasks** scheduled with {func}`asyncio.create_task` copy the current {class}`contextvars.Context` at creation time, so tasks created inside a {class}`~ommx.tracing.capture_trace` block are captured. Tasks created outside the block are not.
- Use {func}`~ommx.tracing.traced` on `async def` functions — it awaits the coroutine inside the trace block, which is what you want.

### Empty span in the text tree / cell output

If a span's duration is listed as `0.0 µs`, the span almost always reached the renderer before it ended (an unterminated `start_as_current_span` call somewhere in your instrumentation). The renderer defends against this with a `0.0` fallback rather than crashing. Check that every span context manager you open is closed; the most common culprit is a manual `tracer.start_span(...)` that was never ended.

### First-call semantics

Do **not** rely on swapping out the active {class}`~opentelemetry.sdk.trace.TracerProvider` after `ommx.tracing` or the Rust → Python tracing bridge has been initialized. Two things lock in on first use and cannot be undone:

1. `ommx.tracing` calls {func}`~opentelemetry.trace.set_tracer_provider` with a fresh {class}`~opentelemetry.sdk.trace.TracerProvider` during {class}`capture_trace.__enter__ <ommx.tracing.capture_trace>` / the first `%%ommx_trace` cell if no provider is set yet. Since Python OTel only honours the first {func}`~opentelemetry.trace.set_tracer_provider` call, a later user-supplied provider is silently ignored.
2. The underlying `pyo3-tracing-opentelemetry` bridge installs its `tracing` subscriber process-wide on the first instrumented Rust call. Spans from later calls continue to flow through that subscriber even if you try to swap providers afterwards.

If OTLP export matters, configure your provider *before* the first OMMX call. If you need to adjust behavior later, mutate the existing SDK provider (for example, {meth}`provider.add_span_processor(new_processor) <opentelemetry.sdk.trace.TracerProvider.add_span_processor>`) rather than replacing it.

## API Reference

Full signatures and docstrings for the symbols discussed above are generated from source in the [autoapi page for `ommx.tracing`](../autoapi/ommx/tracing/index.rst).
