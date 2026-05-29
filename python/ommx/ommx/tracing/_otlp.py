"""OTLP protobuf bridge for stored OMMX traces."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Iterable, Mapping, Sequence, cast

from opentelemetry.exporter.otlp.proto.common.trace_encoder import encode_spans
from opentelemetry.proto.collector.trace.v1.trace_service_pb2 import (
    ExportTraceServiceRequest,
)
from opentelemetry.proto.common.v1.common_pb2 import AnyValue
from opentelemetry.proto.trace.v1.trace_pb2 import Span as ProtoSpan
from opentelemetry.proto.trace.v1.trace_pb2 import Status as ProtoStatus
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import Event, ReadableSpan
from opentelemetry.sdk.util.instrumentation import InstrumentationScope
from opentelemetry.trace import SpanContext, SpanKind, TraceFlags, TraceState
from opentelemetry.trace.status import Status, StatusCode


_SPAN_KIND_FROM_PROTO = {
    ProtoSpan.SPAN_KIND_INTERNAL: SpanKind.INTERNAL,
    ProtoSpan.SPAN_KIND_SERVER: SpanKind.SERVER,
    ProtoSpan.SPAN_KIND_CLIENT: SpanKind.CLIENT,
    ProtoSpan.SPAN_KIND_PRODUCER: SpanKind.PRODUCER,
    ProtoSpan.SPAN_KIND_CONSUMER: SpanKind.CONSUMER,
}


@dataclass
class _StoredReadableSpan:
    name: str
    context: SpanContext
    parent: SpanContext | None
    start_time: int | None
    end_time: int | None
    status: Status
    attributes: Mapping[str, Any] = field(default_factory=dict)
    events: Sequence[Event] = field(default_factory=list)
    kind: SpanKind = SpanKind.INTERNAL
    links: Sequence[Any] = field(default_factory=list)
    resource: Resource = field(default_factory=lambda: Resource.create({}))
    instrumentation_scope: InstrumentationScope | None = None
    dropped_attributes: int = 0
    dropped_events: int = 0
    dropped_links: int = 0

    def get_span_context(self) -> SpanContext:
        return self.context


def spans_to_otlp_protobuf(spans: Iterable[ReadableSpan]) -> bytes:
    """Serialize spans with OpenTelemetry's OTLP protobuf encoder."""

    return encode_spans(list(spans)).SerializeToString()


def spans_from_otlp_protobuf(payload: bytes) -> list[ReadableSpan]:
    """Deserialize OTLP protobuf bytes stored in an OMMX trace layer."""

    request = ExportTraceServiceRequest()
    request.ParseFromString(payload)
    spans: list[ReadableSpan] = []
    for resource_span in request.resource_spans:
        resource = Resource.create(
            _attributes_from_proto(resource_span.resource.attributes),
            schema_url=resource_span.schema_url or None,
        )
        for scope_span in resource_span.scope_spans:
            scope = _instrumentation_scope_from_proto(scope_span)
            for span in scope_span.spans:
                spans.append(_span_from_proto(span, resource, scope))
    return spans


def _span_from_proto(
    span: ProtoSpan,
    resource: Resource,
    instrumentation_scope: InstrumentationScope | None,
) -> ReadableSpan:
    trace_id = _id_from_bytes(span.trace_id)
    context = SpanContext(
        trace_id=trace_id,
        span_id=_id_from_bytes(span.span_id),
        is_remote=False,
        trace_flags=TraceFlags(0),
        trace_state=_trace_state_from_proto(span.trace_state),
    )
    parent = (
        SpanContext(
            trace_id=trace_id,
            span_id=_id_from_bytes(span.parent_span_id),
            is_remote=False,
            trace_flags=TraceFlags(0),
            trace_state=TraceState(),
        )
        if span.parent_span_id
        else None
    )
    return cast(
        ReadableSpan,
        _StoredReadableSpan(
            name=span.name,
            context=context,
            parent=parent,
            start_time=_optional_int(span.start_time_unix_nano),
            end_time=_optional_int(span.end_time_unix_nano),
            status=_status_from_proto(span.status),
            attributes=_attributes_from_proto(span.attributes),
            events=[
                Event(
                    event.name,
                    attributes=_attributes_from_proto(event.attributes),
                    timestamp=_optional_int(event.time_unix_nano),
                )
                for event in span.events
            ],
            kind=_SPAN_KIND_FROM_PROTO.get(span.kind, SpanKind.INTERNAL),
            resource=resource,
            instrumentation_scope=instrumentation_scope,
            dropped_attributes=span.dropped_attributes_count,
            dropped_events=span.dropped_events_count,
            dropped_links=span.dropped_links_count,
        ),
    )


def _instrumentation_scope_from_proto(scope_span: Any) -> InstrumentationScope | None:
    scope = scope_span.scope
    if not scope.name and not scope.version and not scope.attributes:
        return None
    return InstrumentationScope(
        scope.name,
        version=scope.version or None,
        schema_url=getattr(scope_span, "schema_url", "") or None,
        attributes=_attributes_from_proto(scope.attributes),
    )


def _attributes_from_proto(attributes: Sequence[Any]) -> dict[str, Any]:
    return {
        str(attribute.key): _attribute_value_from_proto(attribute.value)
        for attribute in attributes
    }


def _attribute_value_from_proto(value: AnyValue) -> Any:
    field = value.WhichOneof("value")
    if field == "string_value":
        return value.string_value
    if field == "bool_value":
        return value.bool_value
    if field == "int_value":
        return value.int_value
    if field == "double_value":
        return value.double_value
    if field == "array_value":
        return [
            _attribute_value_from_proto(item)
            for item in value.array_value.values
        ]
    if field == "kvlist_value":
        return _attributes_from_proto(value.kvlist_value.values)
    if field == "bytes_value":
        return bytes(value.bytes_value)
    return None


def _status_from_proto(status: ProtoStatus) -> Status:
    if status.code == ProtoStatus.STATUS_CODE_ERROR:
        return Status(StatusCode.ERROR, status.message or None)
    if status.code == ProtoStatus.STATUS_CODE_OK:
        return Status(StatusCode.OK, status.message or None)
    return Status(StatusCode.UNSET)


def _trace_state_from_proto(trace_state: str) -> TraceState:
    if not trace_state:
        return TraceState()
    return TraceState.from_header([trace_state])


def _id_from_bytes(value: bytes) -> int:
    return int.from_bytes(value, byteorder="big") if value else 0


def _optional_int(value: int) -> int | None:
    return value or None
