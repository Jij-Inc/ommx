from dataclasses import dataclass
from typing import Any, ClassVar, cast

import pytest

from ommx.adapter import (
    DIAGNOSTIC_KIND_ANNOTATION,
    DIAGNOSTIC_SCHEMA_ANNOTATION,
    DiagnosticCollector,
    DiagnosticEntry,
    JsonObject,
)


@dataclass(frozen=True, slots=True)
class DummyReport:
    SCHEMA: ClassVar[str] = "org.ommx.test.report.v1"
    NAME: ClassVar[str] = "solver/test/report"
    KIND: ClassVar[str] = "test_report"

    status: str
    value: float | None

    def to_json(self) -> JsonObject:
        return {
            "schema": self.SCHEMA,
            "status": self.status,
            "value": self.value,
        }

    @classmethod
    def from_json(cls, data: JsonObject) -> "DummyReport":
        return cls(
            status=cast(str, data["status"]),
            value=cast(float | None, data["value"]),
        )

    def to_entry(self) -> DiagnosticEntry:
        return DiagnosticEntry.from_json_diagnostic(
            self,
            annotations={"org.ommx.solver.name": "test"},
        )


def test_json_diagnostic_serializes_stable_entry_and_decodes_by_explicit_type():
    report = DummyReport(status="optimal", value=1.5)

    entry = report.to_entry()

    assert entry.name == "solver/test/report"
    assert entry.media_type == "application/json"
    assert (
        entry.data
        == b'{"schema":"org.ommx.test.report.v1","status":"optimal","value":1.5}'
    )
    assert entry.annotations == {
        DIAGNOSTIC_SCHEMA_ANNOTATION: "org.ommx.test.report.v1",
        DIAGNOSTIC_KIND_ANNOTATION: "test_report",
        "org.ommx.solver.name": "test",
    }
    assert entry.decode_as(DummyReport) == report


def test_diagnostic_entry_validates_payload_shape():
    with pytest.raises(ValueError, match="name"):
        DiagnosticEntry("", "application/json", b"{}")

    with pytest.raises(ValueError, match="media_type"):
        DiagnosticEntry("solver/test/report", "", b"{}")

    with pytest.raises(ValueError, match="media type"):
        DiagnosticEntry("solver/test/report", "json", b"{}")

    with pytest.raises(TypeError, match="data"):
        DiagnosticEntry("solver/test/report", "application/json", cast(bytes, "{}"))

    with pytest.raises(TypeError, match="annotations"):
        DiagnosticEntry(
            "solver/test/report",
            "application/json",
            b"{}",
            annotations=cast(dict[str, str], {"key": object()}),
        )


def test_json_diagnostic_rejects_non_json_values_at_serialization():
    @dataclass(frozen=True, slots=True)
    class BadReport:
        SCHEMA: ClassVar[str] = "org.ommx.test.bad.v1"
        NAME: ClassVar[str] = "solver/test/bad"
        KIND: ClassVar[str] = "bad"

        def to_json(self) -> JsonObject:
            return cast(JsonObject, {"bad": object()})

        @classmethod
        def from_json(cls, data: JsonObject) -> "BadReport":
            return cls()

        def to_entry(self) -> DiagnosticEntry:
            return DiagnosticEntry.from_json_diagnostic(self)

    with pytest.raises(TypeError, match="not JSON diagnostic data"):
        BadReport().to_entry()


def test_diagnostic_collector_records_typed_diagnostics_and_entries():
    collector = DiagnosticCollector()
    report = DummyReport(status="optimal", value=None)

    collector.record(report)

    assert collector.diagnostics == (report,)
    (entry,) = collector.entries
    assert entry.decode_as(DummyReport) == report
    with pytest.raises(TypeError, match="JsonDiagnostic"):
        collector.record(cast(Any, object()))
