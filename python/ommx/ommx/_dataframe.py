"""Convert pandas objects to and from OMMX's flat nullable table boundary.

This module owns pandas dtypes, labels, indexes and category metadata. It
exchanges primitive nullable columns and opaque metadata with the binding;
it neither encodes Parquet bytes nor accesses Artifact storage.
"""

from __future__ import annotations

import ast
import datetime as dt
import json
from typing import Any
from zoneinfo import ZoneInfo

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


def _check_axis(original: pd.Index, restored: pd.Index) -> None:
    if original.identical(restored) and not (
        isinstance(original, pd.MultiIndex)
        and isinstance(restored, pd.MultiIndex)
        and not all(a.identical(b) for a, b in zip(original.levels, restored.levels))
    ):
        return
    raise TypeError(
        f"Cannot preserve this {type(original).__name__} in a Parquet DataFrame; "
        "convert the axis to a plain index before saving"
    )


def _columns_index(labels: list[Any], description: dict[str, Any]) -> pd.Index:
    names = [_restore_label(name) for name in description["column_names"]]
    if description["multi_columns"]:
        if not names or any(
            not isinstance(label, tuple) or len(label) != len(names) for label in labels
        ):
            raise ValueError(
                "MultiIndex column labels must match the declared number of levels"
            )
        return pd.MultiIndex.from_tuples(labels, names=names)
    if len(names) != 1:
        raise ValueError("A single column axis must have exactly one name")
    if description["column_range"] is not None:
        result = pd.RangeIndex(**description["column_range"], name=names[0])
        if not result.equals(pd.Index(labels)):
            raise ValueError("RangeIndex metadata does not match column labels")
        return result
    axis = pd.Index(
        labels,
        dtype=description.get("column_dtype"),
        name=names[0],
        tupleize_cols=False,
    )
    if axis.tolist() != labels:
        raise ValueError(
            "Column labels cannot be represented exactly by their declared dtype"
        )
    return axis


def _timezone_spec(dtype: pd.DatetimeTZDtype, field: str) -> dict[str, Any]:
    zone = dtype.tz
    try:
        if isinstance(zone, ZoneInfo):
            if zone.key and ZoneInfo(zone.key) is zone:
                return {"dtype": f"datetime64[{dtype.unit}]", "zoneinfo": zone.key}
        else:
            restored = pd.api.types.pandas_dtype(str(dtype))
            if (
                isinstance(restored, pd.DatetimeTZDtype)
                and restored == dtype
                and str(restored.tz) == str(zone)
            ):
                return {"dtype": str(dtype)}
    except (TypeError, ValueError, KeyError):
        pass
    raise TypeError(
        f"Cannot restore timezone {zone!r} for column {field!r}; "
        "convert it to UTC or a named IANA timezone before saving"
    )


def _string_dtype(missing: str) -> pd.StringDtype:
    if missing == "pd.NA":
        return pd.StringDtype(storage="python")
    if missing != "nan":
        raise ValueError("Unknown string missing-value convention in Parquet metadata")
    try:
        return pd.StringDtype(storage="python", na_value=np.nan)
    except TypeError as error:
        raise TypeError(
            "This pandas version cannot restore a string dtype with NaN semantics"
        ) from error


def _object_null(value: Any) -> bool:
    return value is None or value is pd.NA or value is pd.NaT


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
        if isinstance(dtype, pd.DatetimeTZDtype):
            spec = _timezone_spec(dtype, field)
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
            if dtype.na_value is not pd.NA:
                _string_dtype("nan")
                spec["string_na"] = "nan"
        elif name in _NUMERIC:
            kind = _NUMERIC[name]
        elif name == "object":
            # In object columns NaN is a floating-point value. Remove only
            # explicit null sentinels before inferring the physical type.
            present = [v for v in series if not _object_null(v)]
            inferred = infer_dtype(present, skipna=False)
            kinds = {
                "string": "string",
                "unicode": "string",
                "bytes": "binary",
                "boolean": "boolean",
                "integer": "int64",
                "floating": "float64",
                "mixed-integer-float": "float64",
                "integer-na": "float64",
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
        if name == "object" and kind == "float64":

            def checked_float(value: Any) -> float:
                result = float(value)
                if isinstance(value, (int, np.integer)) and int(value) != result:
                    raise ValueError(
                        f"Integer {value} in column {field!r} cannot be represented exactly as float64"
                    )
                return result

            convert = checked_float
        elif kind == "date":
            epoch = dt.date(1970, 1, 1)

            def days_since_epoch(value: Any) -> int:
                return (value - epoch).days

            convert = days_since_epoch
        else:
            convert = _CASTS[kind]
        if isinstance(dtype, np.dtype) and dtype.kind == "f":
            # A NumPy float has no separate missing-value mask.
            values = series.tolist()
        elif name == "object":
            values = [None if _object_null(v) else convert(v) for v in series]
        else:
            values = [
                None if missing else convert(value)
                for value, missing in zip(series.tolist(), series.isna())
            ]
    return {"name": field, "data": {"type": kind, "values": values}}, spec


def to_columns(df: pd.DataFrame) -> tuple[list[dict[str, Any]], dict[str, str]]:
    """Extract primitive columns and explicitly describe pandas-only structure."""
    if not df.columns.is_unique:
        raise ValueError("Parquet requires unique DataFrame column labels")
    if json.loads(json.dumps(df.attrs, allow_nan=False)) != df.attrs:
        raise TypeError(
            "DataFrame attrs must round-trip as JSON values with string keys"
        )
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

    if isinstance(df.columns, pd.CategoricalIndex):
        raise TypeError(
            "Convert the CategoricalIndex column axis to a plain index before saving"
        )
    for label, series in df.items():
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
            "multi": isinstance(df.index, pd.MultiIndex),
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
        index_levels = []
        for level in range(df.index.nlevels):
            name = df.index.names[level]
            level_values = df.index.get_level_values(level)
            index_levels.append(level_values)
            field = add(
                pd.Series(level_values),
                f"__index_level_{level}__",
                name,
            )
            index_spec["fields"].append(field)
        if isinstance(df.index, pd.MultiIndex):
            _check_axis(
                df.index, pd.MultiIndex.from_arrays(index_levels, names=df.index.names)
            )
        if (
            isinstance(df.index, (pd.DatetimeIndex, pd.TimedeltaIndex))
            and df.index.freq is not None
        ):
            freq = df.index.freqstr
            if pd.tseries.frequencies.to_offset(freq) != df.index.freq:
                raise TypeError(
                    "Cannot preserve the custom frequency of this DataFrame index"
                )
            index_spec["frequency"] = freq
        pandas_index = index_spec["fields"]
    description = {
        "version": 1,
        "columns": labels,
        "column_names": [_label(name) for name in df.columns.names],
        "multi_columns": isinstance(df.columns, pd.MultiIndex),
        "column_dtype": None
        if isinstance(df.columns, pd.MultiIndex)
        else str(df.columns.dtype),
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
    _check_axis(df.columns, _columns_index(list(df.columns), description))
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
    if kind == "null":
        # A Parquet NULL column carries only its row count.
        return pd.Series([None] * data["values"], dtype=object)
    values: list[Any] = data["values"]
    spec = spec or {}
    dtype = spec.get("dtype")
    if dtype == "category":
        definitions = spec["categories"]
        if definitions["type"] != kind:
            raise ValueError("Category metadata does not match the Parquet column type")
        categories = _series(definitions, spec["category_spec"])

        # Match on Parquet values, before converting the dictionary to pandas
        # dates, durations or other dtypes. Only a Parquet null gets code -1.
        def key(value: Any) -> Any:
            return bytes(value) if kind == "binary" else value

        codes = {key(v): i for i, v in enumerate(definitions["values"])}
        try:
            rows = [-1 if v is None else codes[key(v)] for v in values]
        except KeyError as error:
            raise ValueError(
                f"Parquet value {error} is missing from the category metadata"
            ) from error
        return pd.Series(
            pd.Categorical.from_codes(
                rows, categories=categories, ordered=spec["ordered"]
            )
        )
    if dtype in _NUMERIC and _NUMERIC[dtype] != kind:
        raise ValueError(f"pandas dtype {dtype!r} conflicts with Parquet type {kind!r}")
    if kind == "binary":
        values = [None if value is None else bytes(value) for value in values]
    if "timestamp_" in kind:
        unit = {"millis": "ms", "micros": "us", "nanos": "ns"}[kind.rsplit("_", 1)[-1]]
        # An integer array retains full nanosecond precision even with nulls.
        if np.iinfo(np.int64).min in values:
            raise ValueError("Parquet timestamp equals pandas' reserved NaT value")
        raw = np.array(
            [np.iinfo(np.int64).min if value is None else value for value in values],
            dtype=np.int64,
        )
        result = pd.Series(raw.view(f"datetime64[{unit}]"))
        if kind.startswith("utc_"):
            result = result.dt.tz_localize("UTC")
        if dtype and dtype != "object":
            if not dtype.startswith("datetime64["):
                raise ValueError(
                    f"pandas dtype {dtype!r} conflicts with Parquet timestamp"
                )
            target = (
                pd.DatetimeTZDtype(
                    unit=np.datetime_data(dtype)[0], tz=ZoneInfo(spec["zoneinfo"])
                )
                if "zoneinfo" in spec
                else dtype
            )
            restored = result.astype(target)
            if not restored.astype(result.dtype).equals(result):
                raise ValueError(
                    f"Timestamp values cannot be represented exactly as {dtype}"
                )
            return restored
        return result.astype(object) if dtype == "object" else result
    if kind == "date":
        values = [
            None
            if value is None
            else dt.date(1970, 1, 1) + dt.timedelta(days=int(value))
            for value in values
        ]
    if dtype and dtype.startswith("timedelta64["):
        if kind != "int64":
            raise ValueError(f"Duration metadata requires INT64, got {kind!r}")
        unit = dtype[len("timedelta64[") : -1]
        if unit not in {"s", "ms", "us", "ns"}:
            raise ValueError(f"Unsupported timedelta unit in Parquet metadata: {unit}")
        if np.iinfo(np.int64).min in values:
            raise ValueError("Parquet duration equals pandas' reserved NaT value")
        raw = np.array(
            [np.iinfo(np.int64).min if value is None else value for value in values],
            dtype=np.int64,
        )
        return pd.Series(raw.view(dtype))
    if dtype == "object" or kind == "binary" or kind == "date":
        return pd.Series(values, dtype=object)
    if dtype == "string":
        if kind != "string":
            raise ValueError(f"String metadata conflicts with Parquet type {kind!r}")
        return pd.Series(values, dtype=_string_dtype(spec.get("string_na", "pd.NA")))
    has_null = any(v is None for v in values)
    if kind.startswith(("int", "uint")):
        if has_null or (dtype and dtype[0].isupper()):
            # Build from exact integers. Some pandas versions infer a float
            # intermediate from a list of integers containing None.
            mask = np.array([v is None for v in values], dtype=bool)
            integers = np.array([0 if v is None else v for v in values], dtype=kind)
            return pd.Series(pd.arrays.IntegerArray(integers, mask))
        return pd.Series(values, dtype=kind)
    if kind == "boolean":
        return pd.Series(
            values, dtype="boolean" if has_null or dtype == "boolean" else "bool"
        )
    if kind.startswith("float") and (has_null or dtype in ("Float32", "Float64")):
        mask = np.array([value is None for value in values], dtype=bool)
        floats = np.array(values, dtype=kind)
        return pd.Series(pd.arrays.FloatingArray(floats, mask))
    if dtype in _NUMERIC:
        result = pd.Series(values, dtype=dtype)
        if dtype == "float16" and any(
            v is not None and not np.isnan(v) and float(r) != v
            for v, r in zip(values, result)
        ):
            raise ValueError("Parquet float cannot be represented exactly as float16")
        return result
    if kind == "string":
        return pd.Series(values, dtype=object)
    if kind.startswith("float"):
        return pd.Series(values, dtype=kind)
    raise ValueError(f"Unsupported pandas dtype in Parquet metadata: {dtype!r}")


def _validate_field_roles(
    columns: list[dict[str, Any]], description: dict[str, Any]
) -> None:
    physical = {column["name"] for column in columns}
    roles = [item["field"] for item in description["columns"]]
    index = description["index"]
    if index["kind"] == "columns":
        count = len(index["fields"])
        multi = index.get("multi", count > 1)
        if (
            type(multi) is not bool
            or not count
            or len(index["names"]) != count
            or (not multi and count != 1)
        ):
            raise ValueError(
                "Index metadata does not match the number of Parquet index fields"
            )
        roles.extend(index["fields"])
    elif index["kind"] != "range":
        raise ValueError("Unknown index kind in OMMX pandas metadata")
    declared = set(roles)
    if (
        len(roles) != len(declared)
        or declared != physical
        or set(description["specs"]) != physical
    ):
        raise ValueError(
            "OMMX pandas metadata must describe each physical column exactly once as data or index"
        )


def _pyarrow_column_index(
    labels: pd.Index, levels: list[dict[str, Any]]
) -> pd.MultiIndex:
    # PyArrow writes the repr of a tuple of strings, plus each level's dtype.
    parts: list[list[str]] = [[] for _ in levels]
    for label in labels:
        try:
            expression = ast.parse(label, mode="eval").body
        except (TypeError, SyntaxError, ValueError) as error:
            raise ValueError(
                f"Invalid MultiIndex column label in pandas metadata: {label!r}"
            ) from error
        if not isinstance(expression, ast.Tuple) or len(expression.elts) != len(parts):
            raise ValueError(
                f"Invalid MultiIndex column label in pandas metadata: {label!r}"
            )
        for values, item in zip(parts, expression.elts):
            if not isinstance(item, ast.Constant) or not isinstance(item.value, str):
                raise ValueError(
                    f"Invalid MultiIndex column label in pandas metadata: {label!r}"
                )
            values.append(item.value)

    arrays = [
        _pandas_labels(pd.Index(values), level) for values, level in zip(parts, levels)
    ]
    return pd.MultiIndex.from_arrays(arrays, names=[level["name"] for level in levels])


def _pandas_labels(labels: pd.Index, level: dict[str, Any]) -> pd.Index:
    dtype = level.get("numpy_type")
    if dtype in (None, "object") or level.get("pandas_type") == "unicode":
        return labels.rename(level.get("name"))
    if dtype in _NUMERIC and dtype.lower().startswith(("int", "uint", "float")):
        parse = float if dtype.lower().startswith("float") else int
        numbers = [
            parse(label) if isinstance(label, str) else label for label in labels
        ]
        result = pd.Index(numbers, dtype=dtype, name=level.get("name"))
        if result.tolist() == numbers:
            return result
    raise ValueError(f"Unsupported column label dtype in pandas metadata: {dtype!r}")


def from_columns(
    columns: list[dict[str, Any]], metadata: dict[str, str]
) -> pd.DataFrame:
    """Restore pandas structure, or consume standard pandas metadata."""
    if _METADATA_KEY in metadata:
        description = json.loads(metadata[_METADATA_KEY])
        if description["version"] != 1:
            raise ValueError("Unsupported OMMX pandas metadata version")
        _validate_field_roles(columns, description)
        fields = {
            column["name"]: _series(
                column["data"], description["specs"].get(column["name"])
            )
            for column in columns
        }
        labels = description["columns"]
        df = pd.DataFrame({i: fields[item["field"]] for i, item in enumerate(labels)})
        column_labels = [_restore_label(item["label"]) for item in labels]
        df.columns = _columns_index(column_labels, description)
        if not df.columns.is_unique:
            raise ValueError(
                "OMMX metadata maps multiple Parquet fields to the same column label"
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
                if index_spec.get("multi", len(arrays) > 1)
                else pd.Index(arrays[0], name=names[0])
            )
            if index_spec.get("range"):
                declared = pd.RangeIndex(**index_spec["range"], name=names[0])
                if not declared.equals(index):
                    raise ValueError(
                        "RangeIndex metadata does not match Parquet index values"
                    )
                index = declared
            if "frequency" in index_spec:
                if isinstance(index, pd.DatetimeIndex):
                    index = pd.DatetimeIndex(index, freq=index_spec["frequency"])
                elif isinstance(index, pd.TimedeltaIndex):
                    index = pd.TimedeltaIndex(index, freq=index_spec["frequency"])
                else:
                    raise ValueError(
                        "Frequency metadata requires a datetime or timedelta index"
                    )
        if len(index) != len(next(iter(fields.values()), ())):
            raise ValueError("Index metadata does not match the Parquet row count")
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
                "Without ommx.pandas, the full category list and its order cannot be restored losslessly; "
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
        index = pd.RangeIndex(
            spec["start"], spec["stop"], spec["step"], name=spec["name"]
        )
        if len(index_columns) != 1 or len(index) != len(df):
            raise ValueError("Index metadata does not match the Parquet rows or fields")
        df.index = index
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
        df.columns = _pandas_labels(df.columns, column_indexes[0])
    elif len(column_indexes) > 1:
        df.columns = _pyarrow_column_index(df.columns, column_indexes)
    if not df.columns.is_unique:
        raise ValueError(
            "pandas metadata maps multiple Parquet fields to the same column label"
        )
    df.attrs = pandas_metadata.get("attributes", {})
    return df
