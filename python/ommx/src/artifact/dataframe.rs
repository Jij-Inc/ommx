//! pandas is a presentation adapter; the Rust SDK owns Parquet encoding.
use crate::{error::OmmxPyResult, pandas::PyDataFrame};
use ommx::artifact::parquet::{Column, ColumnData, Table};
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyList};
use std::collections::BTreeMap;

pub fn from_pandas(df: &Bound<'_, PyDataFrame>) -> OmmxPyResult<Table> {
    let converted = df
        .py()
        .import("ommx._dataframe")?
        .call_method1("to_columns", (df,))?;
    let (columns, metadata): (Bound<'_, PyList>, BTreeMap<String, String>) = converted.extract()?;
    let mut output = Vec::new();
    for column in columns.iter() {
        let name = column.get_item("name")?.extract()?;
        let data = column.get_item("data")?;
        let kind: String = data.get_item("type")?.extract()?;
        let values = data.get_item("values")?;
        // Extract typed integers directly. Buffering an adjacent-tagged enum
        // through serde's deserialize_any would truncate Python's u64 range.
        let data = match kind.as_str() {
            "boolean" => ColumnData::Boolean(values.extract()?),
            "int8" => ColumnData::Int8(values.extract()?),
            "int16" => ColumnData::Int16(values.extract()?),
            "int32" => ColumnData::Int32(values.extract()?),
            "int64" => ColumnData::Int64(values.extract()?),
            "uint8" => ColumnData::Uint8(values.extract()?),
            "uint16" => ColumnData::Uint16(values.extract()?),
            "uint32" => ColumnData::Uint32(values.extract()?),
            "uint64" => ColumnData::Uint64(values.extract()?),
            "float32" => ColumnData::Float32(values.extract()?),
            "float64" => ColumnData::Float64(values.extract()?),
            "string" => ColumnData::String(values.extract()?),
            "binary" => ColumnData::Binary(values.extract()?),
            "date" => ColumnData::Date(values.extract()?),
            "timestamp_millis" => ColumnData::TimestampMillis(values.extract()?),
            "utc_timestamp_millis" => ColumnData::UtcTimestampMillis(values.extract()?),
            "timestamp_micros" => ColumnData::TimestampMicros(values.extract()?),
            "utc_timestamp_micros" => ColumnData::UtcTimestampMicros(values.extract()?),
            "timestamp_nanos" => ColumnData::TimestampNanos(values.extract()?),
            "utc_timestamp_nanos" => ColumnData::UtcTimestampNanos(values.extract()?),
            _ => {
                return Err(PyRuntimeError::new_err(format!(
                    "Invalid internal DataFrame column type {kind:?}"
                ))
                .into())
            }
        };
        output.push(Column { name, data });
    }
    Ok(Table::new(output, metadata)?)
}

pub fn to_pandas<'py>(py: Python<'py>, table: &Table) -> OmmxPyResult<Bound<'py, PyDataFrame>> {
    let columns = serde_pyobject::to_pyobject(py, table.columns())?;
    let metadata = serde_pyobject::to_pyobject(py, table.metadata())?;
    Ok(py
        .import("ommx._dataframe")?
        .call_method1("from_columns", (columns, metadata))?
        .cast_into::<PyDataFrame>()?)
}
