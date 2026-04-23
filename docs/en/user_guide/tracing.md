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

OMMX emits [OpenTelemetry](https://opentelemetry.io/) spans from the Rust core (via `tracing` + `pyo3-tracing-opentelemetry`) and from selected Python entry points. Two thin wrappers in `ommx.tracing` turn that stream into something you can actually read:

- **`%%ommx_trace`** — a Jupyter cell magic that renders the spans produced during a single cell as a nested text tree, plus a download link for the full trace in Chrome Trace Event Format.
- **`capture_trace` / `@traced`** — a context manager and decorator for the same workflow from plain Python scripts, tests, and CI.

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

1. A nested **text tree** of every span produced in the cell (Rust and Python), annotated with duration and the most useful span attributes.
2. A **download link** for the full trace in [Chrome Trace Event Format](https://docs.google.com/document/d/1CvAClvFfyA5R-PhYUmn5OOQtYMH4h6I0nSsKchNAySU/preview). Drop that JSON file into [Perfetto](https://ui.perfetto.dev/), [speedscope](https://www.speedscope.app/), or `chrome://tracing` to explore the trace as a flame graph.

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

`trace` is a `TraceResult` populated when the block exits:

- `trace.spans` — the raw `list[ReadableSpan]` for custom processing.
- `trace.text_tree()` — the same nested renderer the cell magic uses.
- `trace.chrome_trace_json()` — returns the trace as a JSON string.
- `trace.save_chrome_trace(path)` — writes the JSON to disk (creates parent directories as needed).

If the block raises, `trace.spans` is still populated (with the failing span flagged as `[ERROR]`), so you can inspect or save it from an outer `except` or `finally`. The original exception propagates unchanged — OMMX never swallows.

```{code-cell} ipython3
from pathlib import Path

trace.save_chrome_trace("/tmp/ommx_trace.json")
print(f"Wrote {Path('/tmp/ommx_trace.json').stat().st_size} bytes")
```

### Decorator (`@traced`)

`@traced` is sugar on top of `capture_trace`:

```{code-cell} ipython3
from ommx.tracing import traced

@traced(output="/tmp/evaluate_trace.json")
def evaluate_once(inst):
    return inst.evaluate({0: 1.0, 1: 1.0})

solution = evaluate_once(instance)
print("Wrote trace to /tmp/evaluate_trace.json")
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
- When `output` is set, the Chrome Trace JSON is written **on return *and* on exception** — information is never dropped.
- `async def` is supported. The decorator detects coroutine functions with `inspect.iscoroutinefunction` and awaits them inside the trace block; without that detection, the capture window would close before the coroutine ran and every span would be dropped.

## Span Naming Convention

OMMX relies on `tracing`'s default span names (the bare function name, e.g. `from_bytes`, `evaluate`, `reduce_capabilities`). The fully-qualified module path is carried in the OTel **instrumentation scope** and the span attribute `code.namespace`, so you can still disambiguate two `evaluate` spans from different modules by looking at the scope name or the attributes, not by munging the span name.

When the same method exists on multiple types (for example `Instance.evaluate` vs `SampleSet.evaluate`), the Rust side disambiguates via span **fields** — e.g. `fields(artifact_storage = ...)` — rather than bespoke span names. Those fields show up as OTel attributes in the tree and in the Chrome Trace `args` dict.

(own-tracer-provider)=
## Using Your Own TracerProvider

`ommx.tracing` installs an in-process `TracerProvider` only if none is already registered. If you need spans to flow to an external backend (OTLP, Jaeger, Honeycomb, …), configure your provider **before the first call into the OMMX extension**:

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

1. **OpenTelemetry only honours the first `set_tracer_provider` call.** If you set the provider after the first `Instance.from_bytes(...)` (or any other instrumented call), your provider is ignored. Configure OTel at the very top of your script / notebook.
2. **`ommx.tracing` attaches its collector to whichever provider is active** — it does not replace yours. Spans reach both the OMMX renderer and your OTLP exporter.

If you run with a non-SDK provider that does not support `add_span_processor` (rare, but some vendor SDKs do this), `capture_trace` raises a `RuntimeError` at `__enter__` with a pointer to the fix. Install an `opentelemetry.sdk.trace.TracerProvider` yourself, and add your exporter as another `SpanProcessor` on the same provider.

## Troubleshooting

### I see `(no spans)` in the tree

Most commonly: the traced block didn't actually call into any instrumented OMMX code. The collector captures spans whose `trace_id` falls inside the `capture_trace` window, and only instrumented call sites produce spans — raw Python control flow does not. Double-check that the block contains an actual OMMX call (`Instance.from_bytes`, `Instance.evaluate`, adapter `solve`, etc.).

A second possibility: a non-SDK `TracerProvider` is active and `ommx.tracing` couldn't attach its collector. If that were the case, the first `capture_trace` call would have raised `RuntimeError` — see the message for the remediation.

### My OTLP backend shows the trace but the cell magic shows `(no spans)`

The collector is keyed on `trace_id`. `capture_trace` (and the cell magic) deliberately start with a **fresh** OTel context so the block gets a new `trace_id` — this is what keeps unrelated ambient spans from bleeding into the capture window. That also means spans you start yourself with `tracer.start_as_current_span(..., context=...)` from an unrelated parent won't show up in the cell-magic output, even though they do reach OTLP. Use the cell magic / `capture_trace` block as the outermost span, and nest your own spans inside it.

### Concurrency and async

Inside a `capture_trace` block, spans from the same logical thread nest correctly because OTel propagates the current span via a context variable. A few caveats:

- **Background threads** started *outside* the block do not inherit the block's OTel context. Spans from those threads won't be captured.
- **`asyncio` tasks** scheduled with `asyncio.create_task` copy the current `contextvars.Context` at creation time, so tasks created inside a `capture_trace` block are captured. Tasks created outside the block are not.
- Use `@traced` on `async def` functions — it awaits the coroutine inside the trace block, which is what you want.

### Empty span in the text tree / cell output

If a span's duration is listed as `0.0 µs`, the span almost always reached the renderer before it ended (an unterminated `start_as_current_span` call somewhere in your instrumentation). The renderer defends against this with a `0.0` fallback rather than crashing. Check that every span context manager you open is closed; the most common culprit is a manual `tracer.start_span(...)` that was never ended.

### First-call semantics

The Rust → Python OTel bridge resolves the active `TracerProvider` on each export, so changing providers mid-program is safe — but **the pyo3 extension still caches the tracing subscriber installed on the first call into it**. If you first import `ommx` from a test harness that installs no provider, then try to add an OTLP exporter afterwards, spans from subsequent calls still flow through the already-installed subscriber. Install your provider *before* the first OMMX call when OTLP export matters.
