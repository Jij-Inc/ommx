"""Small OTLP JSON bridge for stored OMMX traces."""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any, Iterable, Mapping, Sequence, cast

from opentelemetry.sdk.trace import ReadableSpan
from opentelemetry.trace.status import Status, StatusCode


@dataclass(frozen=True)
class _StoredSpanContext:
    trace_id: int
    span_id: int


@dataclass
class _StoredEvent:
    name: str
    timestamp: int | None
    attributes: Mapping[str, Any] = field(default_factory=dict)


@dataclass
class _StoredReadableSpan:
    name: str
    context: _StoredSpanContext
    parent: _StoredSpanContext | None
    start_time: int | None
    end_time: int | None
    status: Status
    attributes: Mapping[str, Any] = field(default_factory=dict)
    events: Sequence[_StoredEvent] = field(default_factory=list)


def spans_to_otlp_json(spans: Iterable[ReadableSpan]) -> str:
    """Serialize spans to the OTLP JSON mapping used by trace layers."""

    span_list = list(spans)
    if not span_list:
        return json.dumps({"resourceSpans": []})

    first = span_list[0]
    resource_attrs = getattr(getattr(first, "resource", None), "attributes", {}) or {}
    scope = getattr(first, "instrumentation_scope", None)
    scope_name = getattr(scope, "name", "") or "ommx"

    return json.dumps(
        {
            "resourceSpans": [
                {
                    "resource": {
                        "attributes": _attributes_to_otlp(resource_attrs),
                    },
                    "scopeSpans": [
                        {
                            "scope": {"name": scope_name},
                            "spans": [_span_to_otlp(span) for span in span_list],
                        }
                    ],
                }
            ]
        },
        separators=(",", ":"),
    )


def spans_from_otlp_json(payload: str | bytes) -> list[ReadableSpan]:
    """Deserialize the subset of OTLP JSON produced by :func:`spans_to_otlp_json`."""

    if isinstance(payload, bytes):
        payload = payload.decode("utf-8")
    data = json.loads(payload)
    spans: list[ReadableSpan] = []
    for resource_span in data.get("resourceSpans", []):
        for scope_span in resource_span.get("scopeSpans", []):
            for span in scope_span.get("spans", []):
                spans.append(_span_from_otlp(span))
    return spans


def _span_to_otlp(span: ReadableSpan) -> dict[str, Any]:
    ctx = span.context
    if ctx is None:
        raise ValueError("Cannot serialize a span without context")

    out: dict[str, Any] = {
        "traceId": f"{ctx.trace_id:032x}",
        "spanId": f"{ctx.span_id:016x}",
        "name": span.name,
        "startTimeUnixNano": str(span.start_time or 0),
        "endTimeUnixNano": str(span.end_time or 0),
        "attributes": _attributes_to_otlp(span.attributes or {}),
        "events": [_event_to_otlp(event) for event in span.events],
        "status": _status_to_otlp(span.status),
    }
    if span.parent is not None:
        out["parentSpanId"] = f"{span.parent.span_id:016x}"
    return out


def _span_from_otlp(span: Mapping[str, Any]) -> ReadableSpan:
    trace_id = int(str(span["traceId"]), 16)
    span_id = int(str(span["spanId"]), 16)
    parent_span_id = span.get("parentSpanId")
    parent = (
        _StoredSpanContext(trace_id=trace_id, span_id=int(str(parent_span_id), 16))
        if parent_span_id
        else None
    )
    return cast(
        ReadableSpan,
        _StoredReadableSpan(
            name=str(span.get("name", "")),
            context=_StoredSpanContext(trace_id=trace_id, span_id=span_id),
            parent=parent,
            start_time=_optional_int(span.get("startTimeUnixNano")),
            end_time=_optional_int(span.get("endTimeUnixNano")),
            status=_status_from_otlp(span.get("status", {})),
            attributes=_attributes_from_otlp(span.get("attributes", [])),
            events=[
                _StoredEvent(
                    name=str(event.get("name", "")),
                    timestamp=_optional_int(event.get("timeUnixNano")),
                    attributes=_attributes_from_otlp(event.get("attributes", [])),
                )
                for event in span.get("events", [])
            ],
        ),
    )


def _event_to_otlp(event: Any) -> dict[str, Any]:
    return {
        "name": event.name,
        "timeUnixNano": str(event.timestamp or 0),
        "attributes": _attributes_to_otlp(event.attributes or {}),
    }


def _attributes_to_otlp(attributes: Mapping[str, Any]) -> list[dict[str, Any]]:
    return [
        {"key": str(key), "value": _attribute_value_to_otlp(value)}
        for key, value in attributes.items()
    ]


def _attributes_from_otlp(attributes: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    return {
        str(attribute["key"]): _attribute_value_from_otlp(attribute.get("value", {}))
        for attribute in attributes
    }


def _attribute_value_to_otlp(value: Any) -> dict[str, Any]:
    if isinstance(value, bool):
        return {"boolValue": value}
    if isinstance(value, int):
        return {"intValue": str(value)}
    if isinstance(value, float):
        return {"doubleValue": value}
    if isinstance(value, str):
        return {"stringValue": value}
    if isinstance(value, (list, tuple)):
        return {
            "arrayValue": {
                "values": [_attribute_value_to_otlp(item) for item in value],
            }
        }
    return {"stringValue": str(value)}


def _attribute_value_from_otlp(value: Mapping[str, Any]) -> Any:
    if "stringValue" in value:
        return value["stringValue"]
    if "boolValue" in value:
        return value["boolValue"]
    if "intValue" in value:
        return int(value["intValue"])
    if "doubleValue" in value:
        return float(value["doubleValue"])
    if "arrayValue" in value:
        return [
            _attribute_value_from_otlp(item)
            for item in value["arrayValue"].get("values", [])
        ]
    return None


def _status_to_otlp(status: Status | None) -> dict[str, str]:
    if status is None or status.status_code is StatusCode.UNSET:
        return {"code": "STATUS_CODE_UNSET"}
    if status.status_code is StatusCode.OK:
        return {"code": "STATUS_CODE_OK"}
    return {"code": "STATUS_CODE_ERROR"}


def _status_from_otlp(status: Mapping[str, Any]) -> Status:
    code = status.get("code")
    if code == "STATUS_CODE_ERROR":
        return Status(StatusCode.ERROR)
    if code == "STATUS_CODE_OK":
        return Status(StatusCode.OK)
    return Status(StatusCode.UNSET)


def _optional_int(value: Any) -> int | None:
    if value is None:
        return None
    parsed = int(value)
    return parsed or None
