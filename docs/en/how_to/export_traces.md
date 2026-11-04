# Export spans to an OpenTelemetry backend

Use this setup to send the spans from an OMMX application to your OpenTelemetry collector. For local inspection in a notebook or script, see {mod}`ommx.tracing`. To keep a trace with a recorded Run, see {attr}`~ommx.experiment.SealedRun.trace`.

In the SDK environment install `opentelemetry-exporter-otlp-proto-grpc`. Set `OTEL_EXPORTER_OTLP_ENDPOINT` to the gRPC endpoint for your collector and supply credentials if it requires them. Configure the SDK provider once at the start of the process or notebook kernel, before importing OMMX. If the application already owns a provider, add the span processor to that provider instead.

```python
from opentelemetry import trace
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor

provider = TracerProvider()
provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter()))
trace.set_tracer_provider(provider)

from ommx import Instance
from ommx.tracing import capture_trace

with capture_trace("evaluate candidate") as captured:
    Instance.minimize().evaluate({})
print(captured)
provider.shutdown()
```

The local span collector attaches to the active provider, so the block appears in both the local tree and the external backend. In this standalone script, shutdown flushes the provider at process exit. In a service, leave shutdown to the owner of the provider.
