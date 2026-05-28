import json
from typing import Any, cast

import pytest

from ommx.adapter import DiagnosticCollector, DiagnosticEntry


def test_diagnostic_entry_from_json_encodes_stable_payload():
    entry = DiagnosticEntry.from_json(
        "solver/test/report",
        {"b": 2, "a": 1},
        annotations={"org.ommx.solver.name": "test"},
    )

    assert entry.name == "solver/test/report"
    assert entry.media_type == "application/json"
    assert entry.data == b'{"a":1,"b":2}'
    assert entry.annotations == {"org.ommx.solver.name": "test"}
    assert json.loads(entry.data) == {"a": 1, "b": 2}


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


def test_diagnostic_collector_records_entries():
    collector = DiagnosticCollector()
    entry = DiagnosticEntry("solver/test/log", "text/plain", b"solved")

    collector.record(entry)

    assert collector.entries == (entry,)
    with pytest.raises(TypeError, match="DiagnosticEntry"):
        collector.record(cast(Any, object()))
