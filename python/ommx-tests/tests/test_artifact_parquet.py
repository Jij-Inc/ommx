"""Artifact composition, mutation safety and compatibility at the public API."""

from __future__ import annotations

import datetime as dt
import os
from pathlib import Path
import subprocess
import sys

import pandas as pd
from pandas.testing import assert_frame_equal
import pytest

from ommx.artifact import ArtifactDraft

FIXTURES = Path(__file__).parent / "fixtures" / "parquet"


def legacy_dataframe():
    return pd.DataFrame(
        {
            "integer": pd.array([1, None, -3], dtype="Int64"),
            "unsigned": pd.array([2**64 - 1, None, 1], dtype="UInt64"),
            "small": pd.array([1, None, -3], dtype="Int8"),
            "real": [1.25, None, -2.5],
            "text": ["日本語", None, ""],
            "binary": [b"\x00\xff", None, b""],
            "flag": pd.array([True, None, False], dtype="boolean"),
            "date": [dt.date(2020, 1, 2), None, dt.date(1969, 12, 31)],
            "time": pd.to_datetime(
                ["2020-01-02 03:04:05.123456789", None, "1969-12-31 00:00:00"],
                format="mixed",
            ),
            "zoned": pd.to_datetime(
                ["2020-01-02", None, "1969-12-31"], utc=True
            ).tz_convert("Asia/Tokyo"),
        },
        index=pd.Index(["a", "b", "c"], name="row"),
    )


def test_dataframe_layer_preserves_data_metadata_and_annotations():
    # Exercise the Python/Rust/storage composition once with representative
    # values. Dtype permutations are tested by the conversion owner directly.
    df = legacy_dataframe()
    df["integer"] = pd.array([-(2**63), None, 2**63 - 1], dtype="Int64")
    df["category"] = pd.Categorical(
        ["b", None, "a"], categories=["b", "a", "unused"], ordered=True
    )
    df.attrs = {"title": "results", "version": 1}
    draft = ArtifactDraft.temp()
    descriptor = draft.add_dataframe(df, title="table")
    artifact = draft.commit()
    assert descriptor.media_type == "application/vnd.apache.parquet"
    assert descriptor.annotations["org.ommx.user.title"] == "table"
    result = artifact.get_dataframe(descriptor)
    assert isinstance(result, pd.DataFrame)
    assert_frame_equal(result, df)
    assert result.attrs == df.attrs


def test_failed_dataframe_conversion_preserves_existing_layers():
    draft = ArtifactDraft.temp()
    previous = draft.add_json({"keep": "existing layer"})
    with pytest.raises(TypeError, match="unsupported Parquet"):
        draft.add_dataframe(pd.DataFrame({"value": [[1, 2]]}))
    artifact = draft.commit()
    assert artifact.layers == [previous]
    assert artifact.get_json(previous) == {"keep": "existing layer"}


def test_reads_legacy_pyarrow_parquet():
    draft = ArtifactDraft.temp()
    descriptor = draft.add_layer(
        "application/vnd.apache.parquet",
        (FIXTURES / "pyarrow-22.parquet").read_bytes(),
        {},
    )
    artifact = draft.commit()
    assert_frame_equal(artifact.get_dataframe(descriptor), legacy_dataframe())


def test_parquet_decode_failure_maps_to_runtime_error():
    # This checks the SDK error boundary, not the dependency's parser coverage.
    draft = ArtifactDraft.temp()
    descriptor = draft.add_layer("application/vnd.apache.parquet", b"not parquet", {})
    artifact = draft.commit()
    with pytest.raises(RuntimeError):
        artifact.get_dataframe(descriptor)


def test_roundtrip_without_pyarrow_or_fastparquet(tmp_path):
    script = """
import importlib.abc
import importlib.metadata
import sys

class NoParquetEngine(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split(".")[0] in {"pyarrow", "fastparquet"}:
            raise ImportError("Parquet engine deliberately unavailable")

sys.meta_path.insert(0, NoParquetEngine())
import pandas as pd
from pandas.testing import assert_frame_equal
from ommx.artifact import ArtifactDraft

assert not any(req.lower().startswith("pyarrow") for req in importlib.metadata.requires("ommx"))
df = pd.DataFrame({"value": pd.array([2**64 - 1, None], dtype="UInt64"), "text": ["日本語", None]})
draft = ArtifactDraft.temp()
descriptor = draft.add_dataframe(df)
artifact = draft.commit()
assert_frame_equal(artifact.get_dataframe(descriptor), df)
assert "pyarrow" not in sys.modules
assert "fastparquet" not in sys.modules
"""
    env = os.environ.copy()
    env["OMMX_LOCAL_REGISTRY_ROOT"] = str(tmp_path / "registry")
    subprocess.run([sys.executable, "-c", script], check=True, env=env)
