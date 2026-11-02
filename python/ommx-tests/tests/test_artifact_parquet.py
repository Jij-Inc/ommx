from __future__ import annotations

import datetime as dt
import os
from pathlib import Path
import subprocess
import sys
from typing import Any

import pandas as pd
from pandas.testing import assert_frame_equal
import pytest

from ommx.artifact import ArtifactDraft

FIXTURES = Path(__file__).parent / "fixtures" / "parquet"


def roundtrip(df: pd.DataFrame) -> pd.DataFrame:
    draft = ArtifactDraft.temp()
    descriptor = draft.add_dataframe(df, title="table")
    artifact = draft.commit()
    assert descriptor.media_type == "application/vnd.apache.parquet"
    assert descriptor.annotations["org.ommx.user.title"] == "table"
    assert artifact.get_blob(descriptor).startswith(b"PAR1")
    result = artifact.get_dataframe(descriptor)
    assert isinstance(result, pd.DataFrame)
    return result


@pytest.mark.parametrize(
    "dtype",
    [
        "int8",
        "int16",
        "int32",
        "int64",
        "uint8",
        "uint16",
        "uint32",
        "uint64",
        "float16",
        "float32",
        "float64",
        "bool",
        "Int8",
        "Int16",
        "Int32",
        "Int64",
        "UInt8",
        "UInt16",
        "UInt32",
        "UInt64",
        "Float32",
        "Float64",
        "boolean",
        "string",
    ],
)
def test_primitive_dtypes(dtype):
    values: list[Any] = ["日本語", "", None] if dtype == "string" else [1, 0, 1]
    if dtype[0].isupper() or dtype in {"boolean", "string"}:
        values[-1] = None
    df = pd.DataFrame({"value": pd.Series(values, dtype=dtype)})
    assert_frame_equal(roundtrip(df), df)


def test_extreme_integers_nulls_and_special_floats():
    df = pd.DataFrame(
        {
            "signed": pd.array([-(2**63), 2**63 - 1, None], dtype="Int64"),
            "unsigned": pd.array([2**64 - 1, 2**63 + 1, None], dtype="UInt64"),
            "float": [float("inf"), float("-inf"), float("nan")],
            "bytes": [b"\x00\xff", b"", None],
            "date": [dt.date(1969, 12, 31), dt.date(2020, 1, 2), None],
        }
    )
    assert_frame_equal(roundtrip(df), df)


@pytest.mark.parametrize("unit", ["s", "ms", "us", "ns"])
@pytest.mark.parametrize("timezone", [None, "UTC", "Asia/Tokyo"])
def test_temporal_dtypes(unit, timezone):
    timestamps = pd.to_datetime(
        ["1969-12-31 23:59:59.123456789", None, "2020-01-02 03:04:05.987654321"],
        format="mixed",
    )
    if timezone:
        timestamps = timestamps.tz_localize(timezone)
    timestamps = timestamps.as_unit(unit)
    df = pd.DataFrame(
        {
            "timestamp": timestamps,
            "duration": pd.Series([123, None, -456], dtype=f"timedelta64[{unit}]"),
        }
    )
    assert_frame_equal(roundtrip(df), df)


@pytest.mark.parametrize(
    "categories",
    [
        ["a", "b", "unused"],
        [2**63 + 1, 2**64 - 1, 1],
        pd.to_datetime(["2000-01-01", "2001-01-01", "2002-01-01"]),
    ],
)
def test_categories_preserve_unused_values_and_order(categories):
    df = pd.DataFrame(
        {
            "value": pd.Categorical(
                [categories[1], None, categories[0]],
                categories=categories,
                ordered=True,
            )
        }
    )
    assert_frame_equal(roundtrip(df), df)


@pytest.mark.parametrize(
    "df",
    [
        pd.DataFrame(),
        pd.DataFrame({"empty": pd.Series([], dtype="Int64")}),
        pd.DataFrame(index=pd.RangeIndex(10, 19, 3, name="row")),
        pd.DataFrame(index=pd.RangeIndex(7, 9, 2, name="single")),
        pd.DataFrame({"all_null": [None, None]}),
        pd.DataFrame(
            {"__index_level_0__": [1, 2]}, index=pd.Index(["a", "b"], name="row")
        ),
        pd.DataFrame({"value": [1, 2]}, index=pd.RangeIndex(10, 16, 3, name="row")),
        pd.DataFrame({1: [1, 2], "1": [3, 4]}),
        pd.DataFrame(
            {"value": [1, 2]},
            index=pd.MultiIndex.from_tuples(
                [("a", 1), ("b", 2)], names=["group", "number"]
            ),
        ),
        pd.DataFrame(
            [[1, 2]],
            columns=pd.MultiIndex.from_tuples(
                [("a", 1), ("b", 2)], names=["group", "number"]
            ),
        ),
        pd.DataFrame(
            [[1, 2]], columns=pd.Index([("a", 1), ("b", 2)], tupleize_cols=False)
        ),
    ],
)
def test_indexes_labels_and_empty_tables(df):
    assert_frame_equal(roundtrip(df), df)


@pytest.mark.parametrize(
    "values", [[[1, 2], [3]], [{"a": 1}, {"b": 2}], [complex(1, 2), 0j]]
)
def test_unsupported_values_do_not_add_a_layer(values):
    draft = ArtifactDraft.temp()
    with pytest.raises(TypeError, match="unsupported Parquet"):
        draft.add_dataframe(pd.DataFrame({"value": values}))
    artifact = draft.commit()
    assert artifact.layers == []


def test_duplicate_labels_are_rejected_atomically():
    draft = ArtifactDraft.temp()
    with pytest.raises(ValueError, match="unique DataFrame column labels"):
        draft.add_dataframe(pd.DataFrame([[1, 2]], columns=pd.Index(["same", "same"])))
    assert draft.commit().layers == []


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


def test_reads_legacy_pyarrow_parquet():
    draft = ArtifactDraft.temp()
    descriptor = draft.add_layer(
        "application/vnd.apache.parquet",
        (FIXTURES / "pyarrow-22.parquet").read_bytes(),
        {},
    )
    artifact = draft.commit()
    assert_frame_equal(artifact.get_dataframe(descriptor), legacy_dataframe())


@pytest.mark.parametrize("codec", ["snappy", "gzip", "brotli", "lz4", "zstd", "none"])
def test_reads_compressed_files_and_required_columns(codec):
    draft = ArtifactDraft.temp()
    descriptor = draft.add_layer(
        "application/vnd.apache.parquet",
        (FIXTURES / f"{codec}.parquet").read_bytes(),
        {},
    )
    artifact = draft.commit()
    assert_frame_equal(
        artifact.get_dataframe(descriptor),
        pd.DataFrame({"i": [1, 2, 3], "s": ["日本語", None, ""]}),
    )


def test_reads_empty_legacy_table():
    draft = ArtifactDraft.temp()
    descriptor = draft.add_layer(
        "application/vnd.apache.parquet", (FIXTURES / "empty.parquet").read_bytes(), {}
    )
    assert_frame_equal(draft.commit().get_dataframe(descriptor), pd.DataFrame())


def test_legacy_categories_are_not_silently_lost():
    draft = ArtifactDraft.temp()
    descriptor = draft.add_layer(
        "application/vnd.apache.parquet",
        (FIXTURES / "categorical.parquet").read_bytes(),
        {},
    )
    with pytest.raises(ValueError, match="cannot be restored losslessly"):
        draft.commit().get_dataframe(descriptor)


def test_dataframe_attributes_roundtrip():
    df = pd.DataFrame({"value": [1, 2]})
    df.attrs = {"title": "results", "version": 1}
    result = roundtrip(df)
    assert_frame_equal(result, df)
    assert result.attrs == df.attrs


@pytest.mark.parametrize(
    "payload", [b"not parquet", (FIXTURES / "nested.parquet").read_bytes()]
)
def test_malformed_or_unsupported_parquet_is_runtime_error(payload):
    draft = ArtifactDraft.temp()
    descriptor = draft.add_layer("application/vnd.apache.parquet", payload, {})
    artifact = draft.commit()
    with pytest.raises(RuntimeError):
        artifact.get_dataframe(descriptor)


def test_does_not_use_pandas_parquet_engines(monkeypatch):
    def forbidden(*args, **kwargs):
        raise AssertionError("pandas Parquet engine was used")

    monkeypatch.setattr(pd.DataFrame, "to_parquet", forbidden)
    monkeypatch.setattr(pd, "read_parquet", forbidden)
    df = pd.DataFrame({"value": [1, 2]})
    assert_frame_equal(roundtrip(df), df)


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
