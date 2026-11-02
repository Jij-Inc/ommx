//! Flat, nullable tables stored in Parquet artifact layers.
//!
//! Available with the `parquet` feature. This codec uses the native Rust
//! Parquet implementation without its Arrow feature. It supports primitive
//! boolean, integer, floating-point, UTF-8, binary, date and timestamp columns.
//! Nested, decimal, INT96 and other unsupported schemas are rejected explicitly.
//! Metadata is preserved as strings; interpretation belongs to the consumer.
//!
//! ```
//! use ommx::artifact::parquet::{Column, ColumnData, Table};
//!
//! let table = Table::new(
//!     vec![Column {
//!         name: "objective".into(),
//!         data: ColumnData::Float64(vec![Some(1.5), None]),
//!     }],
//!     Default::default(),
//! )?;
//! let decoded = Table::from_parquet(table.to_parquet()?)?;
//! assert_eq!(decoded, table);
//! # Ok::<_, ommx::Error>(())
//! ```

use crate::{
    artifact::{local_registry::StoredDescriptor, media_types, ArtifactDraft, LocalArtifact},
    Result,
};
use anyhow::{bail, ensure, Context};
use bytes::Bytes;
use parquet::{
    basic::{LogicalType, Repetition, TimeUnit, Type as PhysicalType},
    column::reader::ColumnReader,
    data_type::{
        BoolType, ByteArray, ByteArrayType, DataType, DoubleType, FloatType, Int32Type, Int64Type,
    },
    file::{
        metadata::KeyValue,
        properties::WriterProperties,
        reader::FileReader,
        serialized_reader::SerializedFileReader,
        writer::{SerializedColumnWriter, SerializedFileWriter},
    },
    schema::types::Type,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

/// A primitive nullable column. The variant specifies the Parquet logical type.
///
/// Dates count days from 1970-01-01. Timestamps count the named unit from
/// 1970-01-01T00:00:00; `UtcTimestamp*` variants are adjusted to UTC, while
/// `Timestamp*` variants represent local wall-clock time. A time zone name,
/// if needed, belongs in consumer metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "values", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ColumnData {
    /// Nullable Boolean values, in row order.
    Boolean(Vec<Option<bool>>),
    /// Nullable Int8 values, in row order.
    Int8(Vec<Option<i8>>),
    /// Nullable Int16 values, in row order.
    Int16(Vec<Option<i16>>),
    /// Nullable Int32 values, in row order.
    Int32(Vec<Option<i32>>),
    /// Nullable Int64 values, in row order.
    Int64(Vec<Option<i64>>),
    /// Nullable Uint8 values, in row order.
    Uint8(Vec<Option<u8>>),
    /// Nullable Uint16 values, in row order.
    Uint16(Vec<Option<u16>>),
    /// Nullable Uint32 values, in row order.
    Uint32(Vec<Option<u32>>),
    /// Nullable Uint64 values, in row order.
    Uint64(Vec<Option<u64>>),
    /// Nullable Float32 values, in row order.
    Float32(Vec<Option<f32>>),
    /// Nullable Float64 values, in row order.
    Float64(Vec<Option<f64>>),
    /// Nullable String values, in row order.
    String(Vec<Option<String>>),
    /// Nullable Binary values, in row order.
    Binary(Vec<Option<Vec<u8>>>),
    /// Nullable Date values, in row order.
    Date(Vec<Option<i32>>),
    /// Nullable TimestampMillis values, in row order.
    TimestampMillis(Vec<Option<i64>>),
    /// Nullable UtcTimestampMillis values, in row order.
    UtcTimestampMillis(Vec<Option<i64>>),
    /// Nullable TimestampMicros values, in row order.
    TimestampMicros(Vec<Option<i64>>),
    /// Nullable UtcTimestampMicros values, in row order.
    UtcTimestampMicros(Vec<Option<i64>>),
    /// Nullable TimestampNanos values, in row order.
    TimestampNanos(Vec<Option<i64>>),
    /// Nullable UtcTimestampNanos values, in row order.
    UtcTimestampNanos(Vec<Option<i64>>),
}

/// A named column. Table construction validates names and column lengths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    /// Unique Parquet field name.
    pub name: String,
    /// Nullable values and their logical type.
    pub data: ColumnData,
}

/// A validated flat table with equally sized, uniquely named columns.
///
/// Construction and decoding reject duplicate names, unequal lengths and
/// unsupported Parquet schemas. Metadata is opaque and does not alter the
/// schema or values. A table without columns has zero rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    columns: Vec<Column>,
    metadata: BTreeMap<String, String>,
}

impl Table {
    /// Construct a table, validating names and equal column lengths.
    pub fn new(columns: Vec<Column>, metadata: BTreeMap<String, String>) -> Result<Self> {
        let rows = columns.first().map_or(0, |column| column.data.len());
        let mut names = HashSet::new();
        for column in &columns {
            ensure!(
                names.insert(&column.name),
                "Duplicate Parquet column {:?}",
                column.name
            );
            ensure!(
                column.data.len() == rows,
                "Parquet column {:?} has {} rows; expected {rows}",
                column.name,
                column.data.len()
            );
        }
        Ok(Self { columns, metadata })
    }

    /// Columns in schema order.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Opaque file-level key/value metadata.
    pub fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
    }

    /// Number of rows.
    pub fn num_rows(&self) -> usize {
        self.columns.first().map_or(0, |column| column.data.len())
    }

    /// Encode a Parquet file with optional columns and dictionary encoding.
    pub fn to_parquet(&self) -> Result<Vec<u8>> {
        let fields = self
            .columns
            .iter()
            .map(|column| {
                let (physical, logical) = column.data.parquet_type();
                Type::primitive_type_builder(&column.name, physical)
                    .with_repetition(Repetition::OPTIONAL)
                    .with_logical_type(logical)
                    .build()
                    .map(Arc::new)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let schema = Arc::new(
            Type::group_type_builder("ommx")
                .with_fields(fields)
                .build()?,
        );
        let metadata = self
            .metadata
            .iter()
            .map(|(key, value)| KeyValue::new(key.clone(), value.clone()))
            .collect();
        let properties = Arc::new(
            WriterProperties::builder()
                .set_key_value_metadata(Some(metadata))
                .build(),
        );
        let mut bytes = Vec::new();
        {
            let mut writer = SerializedFileWriter::new(&mut bytes, schema, properties)?;
            let mut group = writer.next_row_group()?;
            for column in &self.columns {
                let mut writer = group
                    .next_column()?
                    .context("Missing Parquet column writer")?;
                column.data.write(&mut writer)?;
                writer.close()?;
            }
            group.close()?;
            writer.close()?;
        }
        Ok(bytes)
    }

    /// Decode a flat Parquet file. Unsupported types fail without dropping columns.
    pub fn from_parquet(bytes: Vec<u8>) -> Result<Self> {
        let reader =
            SerializedFileReader::new(Bytes::from(bytes)).context("Failed to open Parquet data")?;
        let file = reader.metadata().file_metadata();
        let schema = file.schema_descr();
        let mut columns = Vec::new();
        for field in schema.root_schema().get_fields() {
            ensure!(
                field.is_primitive(),
                "Nested Parquet column {:?} is unsupported",
                field.name()
            );
            ensure!(
                field.get_basic_info().repetition() != Repetition::REPEATED,
                "Repeated Parquet column {:?} is unsupported",
                field.name()
            );
            columns.push(Column {
                name: field.name().into(),
                data: empty_column(field)?,
            });
        }
        for group in 0..reader.num_row_groups() {
            let group = reader.get_row_group(group)?;
            for (index, column) in columns.iter_mut().enumerate() {
                let descriptor = schema.column(index);
                let data = group.get_column_reader(index)?;
                let before = column.data.len();
                read_column(&mut column.data, data, descriptor.max_def_level() != 0)?;
                ensure!(
                    column.data.len() - before == usize::try_from(group.metadata().num_rows())?,
                    "Parquet column {:?} row count mismatch",
                    column.name
                );
            }
        }
        let mut metadata = BTreeMap::new();
        for entry in file.key_value_metadata().into_iter().flatten() {
            if let Some(value) = &entry.value {
                metadata.insert(entry.key.clone(), value.clone());
            }
        }
        let table = Self::new(columns, metadata)?;
        ensure!(
            table.num_rows() == usize::try_from(file.num_rows())?,
            "Parquet file row count mismatch"
        );
        Ok(table)
    }
}

impl ColumnData {
    /// Number of values, including nulls.
    pub fn len(&self) -> usize {
        match self {
            Self::Boolean(values) => values.len(),
            Self::Int8(values) => values.len(),
            Self::Int16(values) => values.len(),
            Self::Int32(values) => values.len(),
            Self::Int64(values) => values.len(),
            Self::Uint8(values) => values.len(),
            Self::Uint16(values) => values.len(),
            Self::Uint32(values) => values.len(),
            Self::Uint64(values) => values.len(),
            Self::Float32(values) => values.len(),
            Self::Float64(values) => values.len(),
            Self::String(values) => values.len(),
            Self::Binary(values) => values.len(),
            Self::Date(values) => values.len(),
            Self::TimestampMillis(values) => values.len(),
            Self::UtcTimestampMillis(values) => values.len(),
            Self::TimestampMicros(values) => values.len(),
            Self::UtcTimestampMicros(values) => values.len(),
            Self::TimestampNanos(values) => values.len(),
            Self::UtcTimestampNanos(values) => values.len(),
        }
    }

    /// Whether this column has no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn parquet_type(&self) -> (PhysicalType, Option<LogicalType>) {
        match self {
            Self::Boolean(_) => (PhysicalType::BOOLEAN, None),
            Self::Int8(_) => (PhysicalType::INT32, Some(LogicalType::integer(8, true))),
            Self::Int16(_) => (PhysicalType::INT32, Some(LogicalType::integer(16, true))),
            Self::Int32(_) => (PhysicalType::INT32, Some(LogicalType::integer(32, true))),
            Self::Int64(_) => (PhysicalType::INT64, Some(LogicalType::integer(64, true))),
            Self::Uint8(_) => (PhysicalType::INT32, Some(LogicalType::integer(8, false))),
            Self::Uint16(_) => (PhysicalType::INT32, Some(LogicalType::integer(16, false))),
            Self::Uint32(_) => (PhysicalType::INT32, Some(LogicalType::integer(32, false))),
            Self::Uint64(_) => (PhysicalType::INT64, Some(LogicalType::integer(64, false))),
            Self::Float32(_) => (PhysicalType::FLOAT, None),
            Self::Float64(_) => (PhysicalType::DOUBLE, None),
            Self::String(_) => (PhysicalType::BYTE_ARRAY, Some(LogicalType::String)),
            Self::Binary(_) => (PhysicalType::BYTE_ARRAY, None),
            Self::Date(_) => (PhysicalType::INT32, Some(LogicalType::Date)),
            Self::TimestampMillis(_) => (
                PhysicalType::INT64,
                Some(LogicalType::timestamp(false, TimeUnit::MILLIS)),
            ),
            Self::UtcTimestampMillis(_) => (
                PhysicalType::INT64,
                Some(LogicalType::timestamp(true, TimeUnit::MILLIS)),
            ),
            Self::TimestampMicros(_) => (
                PhysicalType::INT64,
                Some(LogicalType::timestamp(false, TimeUnit::MICROS)),
            ),
            Self::UtcTimestampMicros(_) => (
                PhysicalType::INT64,
                Some(LogicalType::timestamp(true, TimeUnit::MICROS)),
            ),
            Self::TimestampNanos(_) => (
                PhysicalType::INT64,
                Some(LogicalType::timestamp(false, TimeUnit::NANOS)),
            ),
            Self::UtcTimestampNanos(_) => (
                PhysicalType::INT64,
                Some(LogicalType::timestamp(true, TimeUnit::NANOS)),
            ),
        }
    }

    fn write(&self, writer: &mut SerializedColumnWriter<'_>) -> Result<()> {
        match self {
            Self::Boolean(values) => write_values::<BoolType, _>(writer, values, |value| *value),
            Self::Int8(values) => {
                write_values::<Int32Type, _>(writer, values, |value| *value as i32)
            }
            Self::Int16(values) => {
                write_values::<Int32Type, _>(writer, values, |value| *value as i32)
            }
            Self::Int32(values) => write_values::<Int32Type, _>(writer, values, |value| *value),
            Self::Int64(values) => write_values::<Int64Type, _>(writer, values, |value| *value),
            Self::Uint8(values) => {
                write_values::<Int32Type, _>(writer, values, |value| *value as i32)
            }
            Self::Uint16(values) => {
                write_values::<Int32Type, _>(writer, values, |value| *value as i32)
            }
            Self::Uint32(values) => {
                write_values::<Int32Type, _>(writer, values, |value| *value as i32)
            }
            Self::Uint64(values) => {
                write_values::<Int64Type, _>(writer, values, |value| *value as i64)
            }
            Self::Float32(values) => write_values::<FloatType, _>(writer, values, |value| *value),
            Self::Float64(values) => write_values::<DoubleType, _>(writer, values, |value| *value),
            Self::String(values) => write_values::<ByteArrayType, _>(writer, values, |value| {
                ByteArray::from(value.as_str())
            }),
            Self::Binary(values) => write_values::<ByteArrayType, _>(writer, values, |value| {
                ByteArray::from(value.clone())
            }),
            Self::Date(values) => write_values::<Int32Type, _>(writer, values, |value| *value),
            Self::TimestampMillis(values) => {
                write_values::<Int64Type, _>(writer, values, |value| *value)
            }
            Self::UtcTimestampMillis(values) => {
                write_values::<Int64Type, _>(writer, values, |value| *value)
            }
            Self::TimestampMicros(values) => {
                write_values::<Int64Type, _>(writer, values, |value| *value)
            }
            Self::UtcTimestampMicros(values) => {
                write_values::<Int64Type, _>(writer, values, |value| *value)
            }
            Self::TimestampNanos(values) => {
                write_values::<Int64Type, _>(writer, values, |value| *value)
            }
            Self::UtcTimestampNanos(values) => {
                write_values::<Int64Type, _>(writer, values, |value| *value)
            }
        }
    }
}

fn write_values<D: DataType, T>(
    writer: &mut SerializedColumnWriter<'_>,
    values: &[Option<T>],
    convert: impl Fn(&T) -> D::T,
) -> Result<()> {
    let levels: Vec<i16> = values
        .iter()
        .map(|value| i16::from(value.is_some()))
        .collect();
    let values: Vec<D::T> = values
        .iter()
        .filter_map(|value| value.as_ref().map(&convert))
        .collect();
    writer
        .typed::<D>()
        .write_batch(&values, Some(&levels), None)?;
    Ok(())
}

fn empty_column(field: &Type) -> Result<ColumnData> {
    // Compare complete physical/logical pairs so unsupported logical types
    // (e.g. decimal or JSON) are never silently interpreted as plain values.
    let logical = field.get_basic_info().logical_type_ref();
    if field.get_physical_type() == PhysicalType::BOOLEAN && logical.is_none() {
        return Ok(ColumnData::Boolean(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT32
        && logical == Some(&LogicalType::integer(8, true))
    {
        return Ok(ColumnData::Int8(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT32
        && logical == Some(&LogicalType::integer(16, true))
    {
        return Ok(ColumnData::Int16(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT32
        && logical == Some(&LogicalType::integer(32, true))
    {
        return Ok(ColumnData::Int32(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT64
        && logical == Some(&LogicalType::integer(64, true))
    {
        return Ok(ColumnData::Int64(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT32
        && logical == Some(&LogicalType::integer(8, false))
    {
        return Ok(ColumnData::Uint8(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT32
        && logical == Some(&LogicalType::integer(16, false))
    {
        return Ok(ColumnData::Uint16(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT32
        && logical == Some(&LogicalType::integer(32, false))
    {
        return Ok(ColumnData::Uint32(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT64
        && logical == Some(&LogicalType::integer(64, false))
    {
        return Ok(ColumnData::Uint64(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::FLOAT && logical.is_none() {
        return Ok(ColumnData::Float32(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::DOUBLE && logical.is_none() {
        return Ok(ColumnData::Float64(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::BYTE_ARRAY
        && logical == Some(&LogicalType::String)
    {
        return Ok(ColumnData::String(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::BYTE_ARRAY && logical.is_none() {
        return Ok(ColumnData::Binary(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT32 && logical == Some(&LogicalType::Date) {
        return Ok(ColumnData::Date(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT64
        && logical == Some(&LogicalType::timestamp(false, TimeUnit::MILLIS))
    {
        return Ok(ColumnData::TimestampMillis(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT64
        && logical == Some(&LogicalType::timestamp(true, TimeUnit::MILLIS))
    {
        return Ok(ColumnData::UtcTimestampMillis(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT64
        && logical == Some(&LogicalType::timestamp(false, TimeUnit::MICROS))
    {
        return Ok(ColumnData::TimestampMicros(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT64
        && logical == Some(&LogicalType::timestamp(true, TimeUnit::MICROS))
    {
        return Ok(ColumnData::UtcTimestampMicros(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT64
        && logical == Some(&LogicalType::timestamp(false, TimeUnit::NANOS))
    {
        return Ok(ColumnData::TimestampNanos(Vec::new()));
    }
    if field.get_physical_type() == PhysicalType::INT64
        && logical == Some(&LogicalType::timestamp(true, TimeUnit::NANOS))
    {
        return Ok(ColumnData::UtcTimestampNanos(Vec::new()));
    }
    // Unannotated signed integers are common in files from other producers.
    match (field.get_physical_type(), logical) {
        (PhysicalType::INT32, None) => Ok(ColumnData::Int32(Vec::new())),
        (PhysicalType::INT64, None) => Ok(ColumnData::Int64(Vec::new())),
        _ => bail!(
            "Unsupported Parquet type in column {:?}: {field:?}",
            field.name()
        ),
    }
}

fn read_values<D: DataType, T>(
    reader: ColumnReader,
    optional: bool,
    convert: impl Fn(D::T) -> Result<T>,
) -> Result<Vec<Option<T>>> {
    // Read bounded batches instead of reserving a file-declared row count.
    let mut output = Vec::new();
    let mut reader = D::get_column_reader(reader).context("Parquet physical type mismatch")?;
    loop {
        let mut values = Vec::new();
        let mut levels = Vec::new();
        let (records, _, _) =
            reader.read_records(8192, optional.then_some(&mut levels), None, &mut values)?;
        if records == 0 {
            break;
        }
        let mut values = values.into_iter();
        if optional {
            ensure!(levels.len() == records, "Invalid Parquet definition levels");
            for level in levels {
                match level {
                    0 => output.push(None),
                    1 => output.push(Some(convert(
                        values.next().context("Missing Parquet value")?,
                    )?)),
                    _ => bail!("Invalid flat Parquet definition level {level}"),
                }
            }
            ensure!(values.next().is_none(), "Extra Parquet values");
        } else {
            output.extend(
                values
                    .map(|value| convert(value).map(Some))
                    .collect::<Result<Vec<_>>>()?,
            );
        }
    }
    Ok(output)
}

fn read_column(column: &mut ColumnData, reader: ColumnReader, optional: bool) -> Result<()> {
    match column {
        ColumnData::Boolean(values) => {
            values.extend(read_values::<BoolType, _>(reader, optional, Ok)?)
        }
        ColumnData::Int8(values) => {
            values.extend(read_values::<Int32Type, _>(reader, optional, |value| {
                i8::try_from(value).context("Parquet integer exceeds its declared bit width")
            })?)
        }
        ColumnData::Int16(values) => {
            values.extend(read_values::<Int32Type, _>(reader, optional, |value| {
                i16::try_from(value).context("Parquet integer exceeds its declared bit width")
            })?)
        }
        ColumnData::Int32(values) => {
            values.extend(read_values::<Int32Type, _>(reader, optional, Ok)?)
        }
        ColumnData::Int64(values) => {
            values.extend(read_values::<Int64Type, _>(reader, optional, Ok)?)
        }
        ColumnData::Uint8(values) => {
            values.extend(read_values::<Int32Type, _>(reader, optional, |value| {
                u8::try_from(value).context("Parquet integer exceeds its declared bit width")
            })?)
        }
        ColumnData::Uint16(values) => {
            values.extend(read_values::<Int32Type, _>(reader, optional, |value| {
                u16::try_from(value).context("Parquet integer exceeds its declared bit width")
            })?)
        }
        ColumnData::Uint32(values) => {
            values.extend(read_values::<Int32Type, _>(reader, optional, |value| {
                Ok(value as u32)
            })?)
        }
        ColumnData::Uint64(values) => {
            values.extend(read_values::<Int64Type, _>(reader, optional, |value| {
                Ok(value as u64)
            })?)
        }
        ColumnData::Float32(values) => {
            values.extend(read_values::<FloatType, _>(reader, optional, Ok)?)
        }
        ColumnData::Float64(values) => {
            values.extend(read_values::<DoubleType, _>(reader, optional, Ok)?)
        }
        ColumnData::String(values) => values.extend(read_values::<ByteArrayType, _>(
            reader,
            optional,
            |value| {
                String::from_utf8(value.data().to_vec()).context("Invalid UTF-8 in Parquet string")
            },
        )?),
        ColumnData::Binary(values) => values.extend(read_values::<ByteArrayType, _>(
            reader,
            optional,
            |value| Ok(value.data().to_vec()),
        )?),
        ColumnData::Date(values) => {
            values.extend(read_values::<Int32Type, _>(reader, optional, Ok)?)
        }
        ColumnData::TimestampMillis(values) => {
            values.extend(read_values::<Int64Type, _>(reader, optional, Ok)?)
        }
        ColumnData::UtcTimestampMillis(values) => {
            values.extend(read_values::<Int64Type, _>(reader, optional, Ok)?)
        }
        ColumnData::TimestampMicros(values) => {
            values.extend(read_values::<Int64Type, _>(reader, optional, Ok)?)
        }
        ColumnData::UtcTimestampMicros(values) => {
            values.extend(read_values::<Int64Type, _>(reader, optional, Ok)?)
        }
        ColumnData::TimestampNanos(values) => {
            values.extend(read_values::<Int64Type, _>(reader, optional, Ok)?)
        }
        ColumnData::UtcTimestampNanos(values) => {
            values.extend(read_values::<Int64Type, _>(reader, optional, Ok)?)
        }
    }
    Ok(())
}

impl<'reg> ArtifactDraft<'reg> {
    /// Encode and add a flat Parquet table as an artifact layer.
    ///
    /// Encoding finishes before the registry or draft is changed.
    pub fn add_parquet(
        &mut self,
        table: &Table,
        annotations: std::collections::HashMap<String, String>,
    ) -> Result<StoredDescriptor<'reg>> {
        let bytes = table.to_parquet()?;
        self.add_layer_bytes(media_types::PARQUET.into(), bytes, annotations)
    }
}

impl LocalArtifact<'_> {
    /// Read a flat Parquet table from a stored artifact layer.
    pub fn get_parquet(&self, descriptor: &StoredDescriptor<'_>) -> Result<Table> {
        descriptor.ensure_media_type(&media_types::PARQUET.into())?;
        Table::from_parquet(self.get_blob(descriptor)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn table(data: ColumnData) -> Table {
        Table::new(
            vec![Column {
                name: "value".into(),
                data,
            }],
            BTreeMap::from([("consumer".into(), "opaque metadata".into())]),
        )
        .unwrap()
    }

    #[test]
    fn primitive_types_and_nulls_roundtrip() {
        for data in [
            ColumnData::Boolean(vec![Some(true), None]),
            ColumnData::Int8(vec![Some(i8::MIN), None]),
            ColumnData::Int16(vec![Some(i16::MIN), None]),
            ColumnData::Int32(vec![Some(i32::MIN), None]),
            ColumnData::Int64(vec![Some(i64::MIN), None]),
            ColumnData::Uint8(vec![Some(u8::MAX), None]),
            ColumnData::Uint16(vec![Some(u16::MAX), None]),
            ColumnData::Uint32(vec![Some(u32::MAX), None]),
            ColumnData::Uint64(vec![Some(u64::MAX), None]),
            ColumnData::Float32(vec![Some(f32::INFINITY), None]),
            ColumnData::Float64(vec![Some(f64::NEG_INFINITY), None]),
            ColumnData::String(vec![Some("日本語".to_owned()), None]),
            ColumnData::Binary(vec![Some(vec![0, 255, 128]), None]),
            ColumnData::Date(vec![Some(-12345), None]),
            ColumnData::TimestampMillis(vec![Some(-123456789), None]),
            ColumnData::UtcTimestampMillis(vec![Some(-123456789), None]),
            ColumnData::TimestampMicros(vec![Some(-123456789), None]),
            ColumnData::UtcTimestampMicros(vec![Some(-123456789), None]),
            ColumnData::TimestampNanos(vec![Some(-123456789), None]),
            ColumnData::UtcTimestampNanos(vec![Some(-123456789), None]),
        ] {
            let expected = table(data);
            let bytes = expected.to_parquet().unwrap();
            assert_eq!(&bytes[..4], b"PAR1");
            assert_eq!(Table::from_parquet(bytes).unwrap(), expected);
        }
    }

    proptest! {
        #[test]
        fn table_roundtrip(rows in prop::collection::vec(
            (prop::option::of(any::<i64>()), prop::option::of(any::<u64>()),
             prop::option::of(any::<bool>()), prop::option::of(".*")),
            0..100
        )) {
            let columns = vec![
                Column { name: "signed".into(), data: ColumnData::Int64(rows.iter().map(|r| r.0).collect()) },
                Column { name: "unsigned".into(), data: ColumnData::Uint64(rows.iter().map(|r| r.1).collect()) },
                Column { name: "boolean".into(), data: ColumnData::Boolean(rows.iter().map(|r| r.2).collect()) },
                Column { name: "text".into(), data: ColumnData::String(rows.iter().map(|r| r.3.clone()).collect()) },
            ];
            let expected = Table::new(columns, BTreeMap::new()).unwrap();
            prop_assert_eq!(Table::from_parquet(expected.to_parquet().unwrap()).unwrap(), expected);
        }
    }

    #[test]
    fn constructors_reject_inconsistent_tables() {
        let column = Column {
            name: "same".into(),
            data: ColumnData::Boolean(vec![Some(true)]),
        };
        assert!(Table::new(vec![column.clone(), column.clone()], BTreeMap::new()).is_err());
        let mut second = column.clone();
        second.name = "other".into();
        second.data = ColumnData::Boolean(vec![]);
        assert!(Table::new(vec![column, second], BTreeMap::new()).is_err());
        let empty = Table::new(vec![], BTreeMap::new()).unwrap();
        assert_eq!(
            Table::from_parquet(empty.to_parquet().unwrap()).unwrap(),
            empty
        );
        assert!(Table::from_parquet(b"not parquet".to_vec()).is_err());
    }

    #[test]
    fn rejects_unsupported_schema_even_without_rows() {
        for message in [
            "message schema { OPTIONAL group nested { OPTIONAL INT64 value; } }",
            "message schema { REPEATED INT64 repeated; }",
            "message schema { OPTIONAL INT64 amount (DECIMAL(10, 2)); }",
            "message schema { OPTIONAL INT96 legacy_timestamp; }",
        ] {
            let schema = Arc::new(parquet::schema::parser::parse_message_type(message).unwrap());
            let mut bytes = Vec::new();
            SerializedFileWriter::new(&mut bytes, schema, Arc::new(WriterProperties::default()))
                .unwrap()
                .close()
                .unwrap();
            assert!(Table::from_parquet(bytes).is_err(), "{message}");
        }
    }

    #[test]
    fn artifact_roundtrip_and_descriptor_ownership() {
        use crate::artifact::local_registry::LocalRegistry;
        let root = tempfile::tempdir().unwrap();
        let registry = LocalRegistry::open(root.path()).unwrap();
        let mut draft = ArtifactDraft::new_anonymous_in_registry(&registry).unwrap();
        let expected = table(ColumnData::Int64(vec![Some(42), None]));
        let descriptor = draft.add_parquet(&expected, Default::default()).unwrap();
        let artifact = draft.commit().unwrap();
        assert_eq!(artifact.get_parquet(&descriptor).unwrap(), expected);

        let root2 = tempfile::tempdir().unwrap();
        let registry2 = LocalRegistry::open(root2.path()).unwrap();
        let artifact2 = ArtifactDraft::new_anonymous_in_registry(&registry2)
            .unwrap()
            .commit()
            .unwrap();
        assert!(artifact2.get_parquet(&descriptor).is_err());
    }
}
