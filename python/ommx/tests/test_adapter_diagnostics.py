from dataclasses import dataclass
from typing import Any, cast

from ommx.adapter import DiagnosticCollector


@dataclass(frozen=True, slots=True)
class DummyReport:
    status: str
    value: float | None


def test_diagnostic_collector_records_typed_diagnostics():
    collector = DiagnosticCollector()
    report = DummyReport(status="optimal", value=None)

    collector.record(report)

    assert collector.diagnostics == [report]
    assert collector.diagnostics[0] is report


def test_diagnostic_collector_does_not_require_serialization_hooks():
    @dataclass(frozen=True, slots=True)
    class NoSerializationReport:
        value: object

    collector = DiagnosticCollector()
    report = NoSerializationReport(value=object())

    collector.record(report)

    assert collector.diagnostics == [report]


def test_diagnostic_collector_record_is_append_only():
    collector = DiagnosticCollector()
    diagnostic = object()

    collector.record(cast(Any, diagnostic))

    assert collector.diagnostics == [diagnostic]
    assert collector.diagnostics[0] is diagnostic
