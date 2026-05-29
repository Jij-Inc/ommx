"""Completed trace result returned by :class:`capture_trace`."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Union

from opentelemetry.proto.collector.trace.v1.trace_service_pb2 import (
    ExportTraceServiceRequest,
)
from opentelemetry.proto.trace.v1.trace_pb2 import Span as ProtoSpan

from ._otlp import request_from_otlp_protobuf, request_to_otlp_protobuf


@dataclass
class TraceResult:
    """Populated result of a ``capture_trace`` block.

    Filled in by :class:`capture_trace` on ``__exit__`` (including the
    exception path, so the caller can always inspect the trace even
    when the block raised).
    """

    request: ExportTraceServiceRequest = field(
        default_factory=ExportTraceServiceRequest
    )

    @property
    def spans(self) -> list[ProtoSpan]:
        """Flattened OTLP protobuf spans exported in this trace result."""
        return [
            span
            for resource_span in self.request.resource_spans
            for scope_span in resource_span.scope_spans
            for span in scope_span.spans
        ]

    @classmethod
    def from_otlp_protobuf(cls, payload: bytes) -> "TraceResult":
        """Build a trace result from an OMMX trace layer payload."""
        return cls(request=request_from_otlp_protobuf(payload))

    def text_tree(self) -> str:
        """Return the nested text tree."""
        from ._render import render_text_tree

        return render_text_tree(self)

    def otlp_protobuf(self) -> bytes:
        """Return OTLP protobuf bytes stored in Experiment trace layers."""
        return request_to_otlp_protobuf(self.request)

    def chrome_trace_json(self) -> str:
        """Return a Chrome Trace Event Format JSON string."""
        from ._render import chrome_trace_json

        return chrome_trace_json(self)

    def save_chrome_trace(self, path: Union[str, Path]) -> None:
        """Write the Chrome Trace JSON to ``path`` (creating parents as needed).

        Overwrites any existing file. The UTF-8 encoding matches the
        JSON spec and is what Perfetto / speedscope /
        ``chrome://tracing`` all accept.
        """
        p = Path(path)
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(self.chrome_trace_json(), encoding="utf-8")
