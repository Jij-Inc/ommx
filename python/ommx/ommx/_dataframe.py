"""pandas conversion for the Rust Parquet codec (no PyArrow dependency).

Parquet encoding and decoding belong to the Rust SDK. This module owns pandas
dtypes, labels and indexes; it only exchanges primitive nullable columns with
the binding.
"""

from __future__ import annotations

import datetime as dt
import json
from typing import Any

import numpy as np
import pandas as pd
from pandas.api.types import infer_dtype

_METADATA_KEY = "ommx.pandas"
_NUMERIC = {
    "boolean": "boolean",
    "bool": "boolean",
    **{
        f"{prefix}{bits}": f"{prefix.lower()}{bits}"
        for prefix in ("Int", "UInt", "int", "uint")
        for bits in (8, 16, 32, 64)
    },
    **{
        f"{prefix}{bits}": f"float{bits}"
        for prefix in ("Float", "float")
        for bits in (32, 64)
    },
    "float16": "float32",
}
_CASTS = {
    "boolean": bool,
    **{
        f"{prefix}{bits}": int for prefix in ("int", "uint") for bits in (8, 16, 32, 64)
    },
    "float32": float,
    "float64": float,
    "string": str,
    "binary": lambda value: list(bytes(value)),
}


def _label(value: Any) -> Any:
    if isinstance(value, tuple):
        return {"tuple": [_label(item) for item in value]}
    if value is None or isinstance(value, (str, bool, int, float)):
        return value
    raise TypeError(
        f"Parquet labels must be strings, numbers, tuples or None; got {type(value).__name__}"
    )


def _restore_label(value: Any) -> Any:
    if isinstance(value, dict) and set(value) == {"tuple"}:
        return tuple(_restore_label(item) for item in value["tuple"])
    if value is None or isinstance(value, (str, bool, int, float)):
        return value
    raise ValueError("Invalid pandas label in Parquet metadata")


def _column(series: pd.Series, field: str) -> tuple[dict[str, Any], dict[str, Any]]:
    dtype = series.dtype
    spec: dict[str, Any] = {"dtype": str(dtype)}
    if isinstance(dtype, pd.CategoricalDtype):
        categories, category_spec = _column(pd.Series(dtype.categories), field)
        spec = {
            "dtype": "category",
            "categories": categories["data"],
            "category_spec": category_spec,
            "ordered": dtype.ordered,
        }
        # Values use their real logical type; OMMX metadata also preserves
        # unused categories and ordering.
        column = {
            "name": field,
            "data": {
                "type": categories["data"]["type"],
                "values": [
                    None if code == -1 else categories["data"]["values"][code]
                    for code in series.cat.codes
                ],
            },
        }
        return column, spec
    if isinstance(dtype, pd.DatetimeTZDtype) or dtype.kind == "M":
        unit = (
            dtype.unit
            if isinstance(dtype, pd.DatetimeTZDtype)
            else np.datetime_data(str(dtype))[0]
        )
        if unit not in {"s", "ms", "us", "ns"}:
            raise TypeError(f"Unsupported Parquet timestamp unit {unit!r}")
        storage_unit = "ms" if unit == "s" else unit
        raw = series.astype("int64").to_numpy()
        values = [
            None if missing else int(value) * (1000 if unit == "s" else 1)
            for value, missing in zip(raw, series.isna())
        ]
        kind = (
            ("utc_" if isinstance(dtype, pd.DatetimeTZDtype) else "")
            + "timestamp_"
            + {"ms": "millis", "us": "micros", "ns": "nanos"}[storage_unit]
        )
    elif dtype.kind == "m":
        kind = "int64"
        values = [
            None if missing else int(value)
            for value, missing in zip(series.astype("int64").to_numpy(), series.isna())
        ]
    else:
        name = str(dtype)
        if isinstance(dtype, pd.StringDtype):
            kind = "string"
            spec["dtype"] = "string"
        elif name in _NUMERIC:
            kind = _NUMERIC[name]
        elif name == "object":
            inferred = infer_dtype(series, skipna=True)
            kinds = {
                "string": "string",
                "unicode": "string",
                "bytes": "binary",
                "boolean": "boolean",
                "integer": "int64",
                "floating": "float64",
                "mixed-integer-float": "float64",
                "empty": "string",
                "date": "date",
            }
            kind = kinds.get(inferred, "")
            if not kind:
                raise TypeError(
                    f"Column {field!r} contains unsupported Parquet values ({inferred}); convert it to a primitive dtype before saving"
                )
        else:
            raise TypeError(
                f"Column {field!r} has unsupported Parquet dtype {dtype!s}; convert it to a primitive dtype before saving"
            )
        if kind == "date":
            epoch = dt.date(1970, 1, 1)

            def convert(value: Any) -> int:
                return (value - epoch).days
        else:
            convert = _CASTS[kind]
        values = [
            None if missing else convert(value)
            for value, missing in zip(series.tolist(), series.isna())
        ]
    return {"name": field, "data": {"type": kind, "values": values}}, spec


def to_columns(df: pd.DataFrame) -> tuple[list[dict[str, Any]], dict[str, str]]:
    """Extract primitive columns and explicitly describe pandas-only structure."""
    if not df.columns.is_unique:
        raise ValueError("Parquet requires unique DataFrame column labels")
    columns: list[dict[str, Any]] = []
    specs: dict[str, Any] = {}
    labels = []
    pandas_columns = []
    # Physical names are independent of labels, avoiding collisions with index
    # fields and allowing non-string labels.
    used: set[str] = set()

    def add(series: pd.Series, preferred: str, label: Any) -> str:
        field = preferred
        while field in used:
            field = "_" + field
        used.add(field)
        column, spec = _column(series, field)
        columns.append(column)
        specs[field] = spec
        kind = column["data"]["type"]
        pandas_type = {
            "string": "unicode",
            "binary": "bytes",
            "boolean": "bool",
            "date": "date",
        }.get(kind, "datetime" if "timestamp" in kind else kind)
        # Standard pandas metadata enables index/column reconstruction by other
        # Parquet producers. Exact pandas dtype information lives separately.
        numpy_type = str(series.dtype)
        if spec["dtype"] == "category":
            numpy_type = str(series.cat.categories.dtype)
        pandas_columns.append(
            {
                "name": label
                if label is None or isinstance(label, (str, int, float, bool))
                else str(label),
                "field_name": field,
                "pandas_type": pandas_type,
                "numpy_type": numpy_type,
                "metadata": {"timezone": str(series.dt.tz)}
                if isinstance(series.dtype, pd.DatetimeTZDtype)
                else None,
            }
        )
        return field

    for index, (label, series) in enumerate(df.items()):
        field = add(series, str(label), label)
        labels.append({"field": field, "label": _label(label)})
    range_index = isinstance(df.index, pd.RangeIndex) and len(columns) > 0
    if range_index:
        assert isinstance(df.index, pd.RangeIndex)
        index_spec: Any = {
            "kind": "range",
            "name": _label(df.index.name),
            "start": df.index.start,
            "stop": df.index.stop,
            "step": df.index.step,
        }
        pandas_index: Any = [{**index_spec, "name": df.index.name}]
    else:
        index_spec = {
            "kind": "columns",
            "fields": [],
            "names": [_label(name) for name in df.index.names],
            "range": {
                "start": df.index.start,
                "stop": df.index.stop,
                "step": df.index.step,
            }
            if isinstance(df.index, pd.RangeIndex)
            else None,
        }
        for level in range(df.index.nlevels):
            name = df.index.names[level]
            field = add(
                pd.Series(df.index.get_level_values(level)),
                f"__index_level_{level}__",
                name,
            )
            index_spec["fields"].append(field)
        pandas_index = index_spec["fields"]
    description = {
        "version": 1,
        "columns": labels,
        "column_names": [_label(name) for name in df.columns.names],
        "multi_columns": isinstance(df.columns, pd.MultiIndex),
        "column_range": {
            "start": df.columns.start,
            "stop": df.columns.stop,
            "step": df.columns.step,
        }
        if isinstance(df.columns, pd.RangeIndex)
        else None,
        "index": index_spec,
        "specs": specs,
        "attrs": df.attrs,
    }
    pandas_metadata = {
        "index_columns": pandas_index,
        "column_indexes": [
            {
                "name": df.columns.name
                if not isinstance(df.columns, pd.MultiIndex)
                else None,
                "field_name": None,
                "pandas_type": "unicode",
                "numpy_type": "object",
                "metadata": {"encoding": "UTF-8"},
            }
        ],
        "columns": pandas_columns,
        "attributes": df.attrs,
        "creator": {"library": "ommx", "version": "1"},
        "pandas_version": pd.__version__,
    }
    return columns, {
        _METADATA_KEY: json.dumps(description, allow_nan=False),
        "pandas": json.dumps(pandas_metadata, allow_nan=False),
    }


def _series(data: dict[str, Any], spec: dict[str, Any] | None = None) -> pd.Series:
    kind = data["type"]
    values: list[Any] = data["values"]
    spec = spec or {}
    dtype = spec.get("dtype")
    if dtype == "category":
        categories = _series(spec["categories"], spec["category_spec"])
        decoded = _series(data, {"dtype": "object"})
        return pd.Series(
            pd.Categorical(decoded, categories=categories, ordered=spec["ordered"])
        )
    if kind == "binary":
        values = [None if value is None else bytes(value) for value in values]
    if "timestamp_" in kind:
        unit = {"millis": "ms", "micros": "us", "nanos": "ns"}[kind.rsplit("_", 1)[-1]]
        # An integer array retains full nanosecond precision even with nulls.
        raw = np.array(
            [np.iinfo(np.int64).min if value is None else value for value in values],
            dtype=np.int64,
        )
        result = pd.Series(pd.to_datetime(raw, unit=unit, utc=kind.startswith("utc_")))
        if dtype and dtype != "object":
            return result.astype(dtype)
        return result.astype(object) if dtype == "object" else result
    if kind == "date":
        values = [
            None
            if value is None
            else dt.date(1970, 1, 1) + dt.timedelta(days=int(value))
            for value in values
        ]
    if dtype and dtype.startswith("timedelta64["):
        unit = dtype[len("timedelta64[") : -1]
        if unit not in {"s", "ms", "us", "ns"}:
            raise ValueError(f"Unsupported timedelta unit in Parquet metadata: {unit}")
        raw = np.array(
            [np.iinfo(np.int64).min if value is None else value for value in values],
            dtype=np.int64,
        )
        return pd.Series(raw.view(dtype))
    if dtype == "object" or kind == "binary" or kind == "date":
        return pd.Series(values, dtype=object)
    if dtype == "string":
        return pd.Series(values, dtype=pd.StringDtype(storage="python"))
    if dtype in _NUMERIC:
        if any(value is None for value in values) and dtype in {
            "bool",
            *[
                f"{prefix}{bits}"
                for prefix in ("int", "uint")
                for bits in (8, 16, 32, 64)
            ],
        }:
            # Legacy pandas/Arrow metadata keeps NumPy's non-nullable dtype
            # even when Parquet has nulls; match pandas' normal promotion.
            return pd.Series(values, dtype=object if dtype == "bool" else "float64")
        return pd.Series(values, dtype=dtype)
    if kind in {"boolean", "string"}:
        return pd.Series(values, dtype=object)
    if kind in _NUMERIC:
        return pd.Series(
            values,
            dtype=kind
            if not any(value is None for value in values) or kind.startswith("float")
            else "float64",
        )
    raise ValueError(f"Unsupported pandas dtype in Parquet metadata: {dtype!r}")


def from_columns(
    columns: list[dict[str, Any]], metadata: dict[str, str]
) -> pd.DataFrame:
    """Restore pandas structure, or consume standard pandas metadata."""
    if _METADATA_KEY in metadata:
        description = json.loads(metadata[_METADATA_KEY])
        if description["version"] != 1:
            raise ValueError("Unsupported OMMX pandas metadata version")
        fields = {
            column["name"]: _series(
                column["data"], description["specs"].get(column["name"])
            )
            for column in columns
        }
        labels = description["columns"]
        df = pd.DataFrame({i: fields[item["field"]] for i, item in enumerate(labels)})
        column_labels = [_restore_label(item["label"]) for item in labels]
        column_names = [_restore_label(name) for name in description["column_names"]]
        df.columns = (
            pd.MultiIndex.from_tuples(column_labels, names=column_names)
            if description["multi_columns"]
            else pd.Index(column_labels, name=column_names[0], tupleize_cols=False)
        )
        if description.get("column_range") is not None:
            df.columns = pd.RangeIndex(
                **description["column_range"], name=column_names[0]
            )
        index_spec = description["index"]
        if index_spec["kind"] == "range":
            index = pd.RangeIndex(
                index_spec["start"],
                index_spec["stop"],
                index_spec["step"],
                name=_restore_label(index_spec["name"]),
            )
        else:
            arrays = [fields[field] for field in index_spec["fields"]]
            names = [_restore_label(name) for name in index_spec["names"]]
            index = (
                pd.MultiIndex.from_arrays(arrays, names=names)
                if len(arrays) > 1
                else pd.Index(arrays[0], name=names[0])
            )
            if index_spec.get("range"):
                index = pd.RangeIndex(**index_spec["range"], name=names[0])
        if not labels:
            df = pd.DataFrame(index=index, columns=df.columns)
        else:
            df.index = index
        df.attrs = description.get("attrs", {})
        return df

    pandas_metadata = json.loads(metadata.get("pandas", "{}"))
    specs = {item["field_name"]: item for item in pandas_metadata.get("columns", [])}
    fields = {}
    for column in columns:
        info = specs.get(column["name"], {})
        dtype = info.get("numpy_type")
        timezone = (info.get("metadata") or {}).get("timezone")
        if timezone and dtype and dtype.startswith("datetime64["):
            dtype = f"datetime64[{dtype[len('datetime64[') : -1]}, {timezone}]"
        if info.get("pandas_type") == "categorical":
            raise ValueError(
                "Legacy categorical Parquet metadata cannot be restored losslessly by the Rust backend; "
                "read it with pandas.read_parquet and PyArrow, then save the DataFrame with add_dataframe"
            )
        series = _series(column["data"], {"dtype": dtype})
        if timezone and "timestamp" in column["data"]["type"]:
            series = (
                series.dt.tz_convert(timezone)
                if series.dt.tz is not None
                else series.dt.tz_localize("UTC").dt.tz_convert(timezone)
            )
        fields[column["name"]] = series
    df = pd.DataFrame(fields)
    index_columns = pandas_metadata.get("index_columns", [])
    if index_columns and isinstance(index_columns[0], dict):
        spec = index_columns[0]
        df.index = pd.RangeIndex(
            spec["start"], spec["stop"], spec["step"], name=spec["name"]
        )
    elif index_columns:
        df = df.set_index(index_columns)
        df.index.names = [specs.get(field, {}).get("name") for field in index_columns]
    names = {
        field: info["name"] for field, info in specs.items() if field in df.columns
    }
    if names:
        df = df.rename(columns=names)
    column_indexes = pandas_metadata.get("column_indexes", [])
    if len(column_indexes) == 1:
        df.columns.name = column_indexes[0].get("name")
    df.attrs = pandas_metadata.get("attributes", {})
    return df
