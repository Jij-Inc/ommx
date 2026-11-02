"""pandas dtype, label and metadata contracts, without Parquet or a registry."""

from __future__ import annotations

import datetime as dt
import json
from typing import Any

import numpy as np
import pandas as pd
from pandas.testing import assert_frame_equal
import pytest

from ommx._dataframe import from_columns, to_columns


def convert_roundtrip(df: pd.DataFrame) -> pd.DataFrame:
    columns, metadata = to_columns(df)
    return from_columns(columns, metadata)


@pytest.mark.parametrize(
    "value", [2**53 + 1, np.int64(-(2**53 + 1)), np.uint64(2**64 - 1)]
)
def test_mixed_numeric_object_column_rejects_lossy_float_conversion(value):
    with pytest.raises(ValueError, match="cannot be represented exactly"):
        to_columns(pd.DataFrame({"v": pd.Series([value, 0.5], dtype=object)}))
    exact = pd.DataFrame({"v": pd.Series([2**53 + 2, 0.5], dtype=object)})
    assert_frame_equal(convert_roundtrip(exact), exact)


@pytest.mark.parametrize("change", ["added", "removed", "duplicate", "index_overlap"])
def test_metadata_must_account_for_every_physical_field_once(change):
    frame = pd.DataFrame({"a": [1], "b": [2]}, index=pd.Index(["x"]))
    columns, metadata = to_columns(frame)
    desc = json.loads(metadata["ommx.pandas"])
    if change == "added":
        columns.append({"name": "c", "data": {"type": "int64", "values": [3]}})
    elif change == "removed":
        columns.pop()
    elif change == "duplicate":
        desc["columns"].append(desc["columns"][0])
    else:
        desc["index"]["fields"] = ["a"]
    metadata["ommx.pandas"] = json.dumps(desc)
    with pytest.raises(ValueError, match="exactly once"):
        from_columns(columns, metadata)


@pytest.mark.parametrize("dtype", ["float32", "float64"])
def test_nullable_floats_preserve_valid_nan_and_null(dtype):
    values = pd.arrays.FloatingArray(
        np.array([np.nan, np.nan, 1], dtype=dtype), np.array([False, True, False])
    )
    frame = pd.DataFrame({"v": values})
    result = convert_roundtrip(frame)
    assert_frame_equal(result, frame)
    assert result["v"].isna().tolist() == [False, True, False]
    assert np.isnan(result["v"].iloc[0])


@pytest.mark.parametrize("unit", ["ms", "us"])
@pytest.mark.parametrize("zoned", [False, True])
def test_timestamp_outside_nanosecond_range(unit, zoned):
    series = pd.Series(["2500-01-01", None], dtype=f"datetime64[{unit}]")
    if zoned:
        series = series.dt.tz_localize("Asia/Tokyo")
    frame = pd.DataFrame({"v": series})
    assert_frame_equal(convert_roundtrip(frame), frame)


def test_standard_pandas_metadata_restores_multiindex_level_values_and_names():
    labels = [("x'y", 1), ("x'y", 2)]
    fields = [repr((text, str(number))) for text, number in labels]
    metadata = {
        "pandas": json.dumps(
            {
                "columns": [
                    {"field_name": name, "name": name, "numpy_type": "int64"}
                    for name in fields
                ],
                "column_indexes": [
                    {"name": "group", "pandas_type": "unicode", "numpy_type": "object"},
                    {"name": "key", "pandas_type": "int64", "numpy_type": "int64"},
                ],
            }
        )
    }
    columns = [
        {"name": name, "data": {"type": "int64", "values": [i]}}
        for i, name in enumerate(fields)
    ]
    expected = pd.DataFrame(
        [[0, 1]], columns=pd.MultiIndex.from_tuples(labels, names=["group", "key"])
    )
    assert_frame_equal(from_columns(columns, metadata), expected)


def test_legacy_logical_null_column_restores_object_dtype_and_length():
    expected = pd.DataFrame({"empty": pd.Series([None, None], dtype=object)})
    assert_frame_equal(
        from_columns([{"name": "empty", "data": {"type": "null", "values": 2}}], {}),
        expected,
    )


@pytest.mark.parametrize("kind,value", [("int64", -(2**63) + 1), ("uint64", 2**64 - 1)])
@pytest.mark.parametrize("pandas_hint", [False, True])
def test_nullable_parquet_integers_are_exact_with_or_without_pandas_metadata(
    kind, value, pandas_hint
):
    metadata = (
        {
            "pandas": json.dumps(
                {"columns": [{"field_name": "id", "name": "id", "numpy_type": kind}]}
            )
        }
        if pandas_hint
        else {}
    )
    result = from_columns(
        [{"name": "id", "data": {"type": kind, "values": [value, None]}}], metadata
    )
    assert int(result["id"].iloc[0]) == value
    assert result["id"].isna().tolist() == [False, True]


def test_pandas_hint_cannot_narrow_a_parquet_integer():
    data = [{"name": "v", "data": {"type": "uint64", "values": [2**64 - 1]}}]
    metadata = {
        "pandas": json.dumps(
            {"columns": [{"field_name": "v", "name": "v", "numpy_type": "float64"}]}
        )
    }
    with pytest.raises(ValueError, match="conflicts with Parquet"):
        from_columns(data, metadata)


def test_category_edit_cannot_turn_an_unknown_value_into_null():
    columns, metadata = to_columns(pd.DataFrame({"v": pd.Categorical(["low", "high"])}))
    columns[0]["data"]["values"][0] = "new"
    with pytest.raises(ValueError, match="missing from the category"):
        from_columns(columns, metadata)


def test_parquet_nan_and_null_are_distinct_even_with_a_numpy_dtype_hint():
    values = [np.nan, None]
    metadata = {
        "pandas": json.dumps(
            {"columns": [{"field_name": "v", "name": "v", "numpy_type": "float64"}]}
        )
    }
    result = from_columns(
        [{"name": "v", "data": {"type": "float64", "values": values}}], metadata
    )
    assert result["v"].isna().tolist() == [False, True]
    assert np.isnan(result["v"].iloc[0])
    columns, _ = to_columns(pd.DataFrame({"v": [np.nan]}))
    assert np.isnan(columns[0]["data"]["values"][0])


@pytest.mark.parametrize("dtype, label", [("int64", 1), ("float64", 0.25)])
def test_pandas_numeric_column_labels(dtype, label):
    field = str(label)
    columns = [{"name": field, "data": {"type": "int64", "values": [7]}}]
    metadata = {
        "pandas": json.dumps({"column_indexes": [{"name": "key", "numpy_type": dtype}]})
    }
    result = from_columns(columns, metadata)
    assert result[label].tolist() == [7]
    assert result.columns.name == "key"


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
    assert_frame_equal(convert_roundtrip(df), df)


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
    assert_frame_equal(convert_roundtrip(df), df)


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
    assert_frame_equal(convert_roundtrip(df), df)


@pytest.mark.parametrize(
    "categories",
    [
        ["a", "b", "unused"],
        [2**63 + 1, 2**64 - 1, 1],
        pd.to_datetime(["2000-01-01", "2001-01-01", "2002-01-01"]),
        pd.to_timedelta(["1 day", "2 days", "3 days"]),
        [b"b", b"a", b"unused"],
    ],
)
@pytest.mark.parametrize(
    "index", [pd.RangeIndex(4, 10, 2), pd.Index(["one", "two", "three"], name="row")]
)
def test_categories_preserve_unused_values_and_order(categories, index):
    df = pd.DataFrame(
        {
            "value": pd.Categorical(
                [categories[1], None, categories[0]],
                categories=categories,
                ordered=True,
            )
        },
        index=index,
    )
    assert_frame_equal(convert_roundtrip(df), df)


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
        pd.DataFrame([[1, 2]], columns=pd.Index([2, 4], dtype="int8")),
        pd.DataFrame(
            {"value": [1, 2]}, index=pd.date_range("2020", periods=2, freq="3D")
        ),
        pd.DataFrame(
            {"value": [1, 2]}, index=pd.timedelta_range("1 day", periods=2, freq="12h")
        ),
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
    assert_frame_equal(convert_roundtrip(df), df)


@pytest.mark.parametrize(
    "values", [[[1, 2], [3]], [{"a": 1}, {"b": 2}], [complex(1, 2), 0j]]
)
def test_unsupported_values_are_rejected(values):
    with pytest.raises(TypeError, match="unsupported Parquet"):
        to_columns(pd.DataFrame({"value": values}))


def test_duplicate_labels_are_rejected():
    with pytest.raises(ValueError, match="unique DataFrame column labels"):
        to_columns(pd.DataFrame([[1, 2]], columns=pd.Index(["same", "same"])))


def test_column_axis_that_cannot_be_preserved_is_rejected_before_storage():
    axis = pd.CategoricalIndex(["A", "B"], categories=["C", "B", "A"], ordered=True)
    with pytest.raises(TypeError, match="plain index"):
        to_columns(pd.DataFrame([[1, 2]], columns=axis))


def test_legacy_categories_are_not_silently_lost():
    columns = [{"name": "category", "data": {"type": "string", "values": ["b", "a"]}}]
    metadata = {
        "pandas": json.dumps(
            {
                "columns": [
                    {
                        "name": "category",
                        "field_name": "category",
                        "pandas_type": "categorical",
                        "numpy_type": "int8",
                        "metadata": {"num_categories": 3, "ordered": True},
                    }
                ]
            }
        )
    }
    with pytest.raises(ValueError, match="cannot be restored losslessly"):
        from_columns(columns, metadata)


def test_empty_legacy_metadata_restores_dataframe_axes():
    metadata = {
        "pandas": json.dumps(
            {
                "index_columns": [
                    {"kind": "range", "name": None, "start": 0, "stop": 0, "step": 1}
                ],
                "columns": [],
                "column_indexes": [{"name": None}],
            }
        )
    }
    assert_frame_equal(from_columns([], metadata), pd.DataFrame())


def test_dataframe_attributes_roundtrip():
    df = pd.DataFrame({"value": [1, 2]})
    df.attrs = {"title": "results", "version": 1}
    result = convert_roundtrip(df)
    assert_frame_equal(result, df)
    assert result.attrs == df.attrs


def test_attribute_that_changes_during_json_encoding_is_rejected():
    frame = pd.DataFrame({"v": [1]})
    frame.attrs["limits"] = (1, 2)
    with pytest.raises(TypeError, match="JSON values"):
        to_columns(frame)


def test_unknown_metadata_version_is_rejected():
    columns, metadata = to_columns(pd.DataFrame({"value": [1]}))
    description = json.loads(metadata["ommx.pandas"])
    description["version"] = 2
    metadata["ommx.pandas"] = json.dumps(description)
    with pytest.raises(ValueError, match="Unsupported OMMX pandas metadata version"):
        from_columns(columns, metadata)
