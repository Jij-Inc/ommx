"""OTLP protobuf codec for stored OMMX traces."""

from __future__ import annotations

from collections.abc import Iterable

from opentelemetry.exporter.otlp.proto.common.trace_encoder import encode_spans
from opentelemetry.proto.collector.trace.v1.trace_service_pb2 import (
    ExportTraceServiceRequest,
)
from opentelemetry.sdk.trace import ReadableSpan


def spans_to_otlp_request(spans: Iterable[ReadableSpan]) -> ExportTraceServiceRequest:
    """Export spans with OpenTelemetry's OTLP protobuf encoder."""

    return encode_spans(list(spans))


def request_to_otlp_protobuf(request: ExportTraceServiceRequest) -> bytes:
    """Serialize an OTLP export request to protobuf bytes."""

    return request.SerializeToString()


def request_from_otlp_protobuf(payload: bytes) -> ExportTraceServiceRequest:
    """Parse OTLP protobuf bytes stored in an OMMX trace layer."""

    request = ExportTraceServiceRequest()
    request.ParseFromString(payload)
    return request
