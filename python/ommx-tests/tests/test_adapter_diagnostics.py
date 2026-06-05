from dataclasses import dataclass

from ommx.adapter import DiagnosticCollector


@dataclass(frozen=True, slots=True)
class DummyReport:
    status: str
    value: float | None


def test_diagnostic_collector_records_typed_diagnostics():
    collector = DiagnosticCollector()
    report = DummyReport(status="optimal", value=None)

    collector.record(report)

    assert collector.diagnostics == (report,)
    assert collector.diagnostics[0] is report


def test_diagnostic_collector_does_not_require_serialization_hooks():
    @dataclass(frozen=True, slots=True)
    class NoSerializationReport:
        value: float

    collector = DiagnosticCollector()
    report = NoSerializationReport(value=float("inf"))

    collector.record(report)

    assert collector.diagnostics == (report,)
