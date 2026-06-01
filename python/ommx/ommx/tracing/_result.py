"""Completed trace result returned by :class:`capture_trace`."""

from __future__ import annotations

from dataclasses import dataclass, field

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
        """Build a trace result from an OMMX trace payload."""
        return cls(request=request_from_otlp_protobuf(payload))

    def otlp_protobuf(self) -> bytes:
        """Return OTLP protobuf bytes stored in Experiment traces."""
        return request_to_otlp_protobuf(self.request)
