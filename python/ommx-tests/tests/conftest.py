"""Pytest configuration for the ommx-tests package.

Sets up an OpenTelemetry ``TracerProvider`` with an in-memory
``SpanExporter`` at the start of the test session so that tests can
assert on spans forwarded from the Rust side via
``pyo3-tracing-opentelemetry``.

The tracing bridge is initialized once per process on the first call to
an instrumented Rust entry point. The autouse session fixture below
guarantees the provider is installed before any test triggers that
initialization.
"""

from __future__ import annotations

from typing import Sequence

import pytest
from opentelemetry import trace
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import ReadableSpan, TracerProvider
from opentelemetry.sdk.trace.export import (
    SimpleSpanProcessor,
    SpanExporter,
    SpanExportResult,
)


class InMemorySpanExporter(SpanExporter):
    """Collects exported spans in a list for assertion from tests."""

    def __init__(self) -> None:
        self.spans: list[ReadableSpan] = []

    def export(self, spans: Sequence[ReadableSpan]) -> SpanExportResult:
        self.spans.extend(spans)
        return SpanExportResult.SUCCESS

    def shutdown(self) -> None:
        return None

    def force_flush(self, timeout_millis: int = 30000) -> bool:  # noqa: ARG002
        return True

    def clear(self) -> None:
        self.spans = []


_test_exporter: InMemorySpanExporter | None = None
_test_provider: TracerProvider | None = None


def get_test_exporter() -> InMemorySpanExporter:
    assert _test_exporter is not None, "Test tracing not initialized"
    return _test_exporter


def get_test_provider() -> TracerProvider:
    assert _test_provider is not None, "Test tracing not initialized"
    return _test_provider


@pytest.fixture(scope="session", autouse=True)
def setup_test_tracing():
    """Install an OTel provider for the session so Rust spans are collected."""
    global _test_exporter, _test_provider

    previous_provider = trace.get_tracer_provider()

    _test_exporter = InMemorySpanExporter()
    resource = Resource.create({"service.name": "ommx-tests"})
    _test_provider = TracerProvider(resource=resource)
    _test_provider.add_span_processor(SimpleSpanProcessor(_test_exporter))
    trace.set_tracer_provider(_test_provider)

    try:
        yield
    finally:
        if _test_provider is not None:
            _test_provider.shutdown()
        trace.set_tracer_provider(previous_provider)
        _test_exporter = None
        _test_provider = None
