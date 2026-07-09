//! Run parameter values and committed run-parameter table serialization.

use super::RunEntry;
use anyhow::{ensure, Context, Result};
use rmpv::Value as MessagePackValue;
use serde::{Deserialize, Serialize};
use std::collections::{btree_map::Values, BTreeMap};
use std::io::Cursor;

/// A scalar cell value accepted by the run parameter table.
///
/// Parameters are intended to become DataFrame / Arrow-like columns at
/// commit time, so this intentionally excludes nulls and structured
/// values. Missing cells are represented by the absence of a
/// `(run_id, value)` entry in the committed column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl From<bool> for ParameterValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

macro_rules! impl_parameter_value_from_signed_integer {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for ParameterValue {
                fn from(value: $ty) -> Self {
                    Self::Int(i64::from(value))
                }
            }
        )*
    };
}

impl_parameter_value_from_signed_integer!(i8, i16, i32, i64);

impl From<f32> for ParameterValue {
    fn from(value: f32) -> Self {
        Self::Float(f64::from(value))
    }
}

impl From<f64> for ParameterValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<String> for ParameterValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for ParameterValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl ParameterValue {
    fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int64",
            Self::Float(_) => "float64",
            Self::String(_) => "string",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParameterSet {
    values: BTreeMap<String, ParameterValue>,
}

impl ParameterSet {
    pub fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, name: String, value: ParameterValue) -> Result<()> {
        self.values.insert(name, value);
        Ok(())
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &ParameterValue)> {
        self.values.iter()
    }
}

#[derive(Debug, Clone, Default)]
pub struct RunParameterTable {
    columns: BTreeMap<String, RunParameterColumn>,
}

impl RunParameterTable {
    pub fn from_runs<'reg>(runs: Values<'_, u64, RunEntry<'reg>>) -> Result<Self> {
        let mut columns = BTreeMap::new();
        for run in runs {
            for (name, value) in run.parameters.iter() {
                columns
                    .entry(name.clone())
                    .or_insert_with(|| RunParameterColumn::from_value(value))
                    .insert(name, run.run_id, value)?;
            }
        }
        Ok(Self { columns })
    }

    pub fn cells(&self) -> Vec<RunParameterCell> {
        self.columns
            .iter()
            .flat_map(|(name, column)| column.cells(name))
            .collect()
    }

    pub(crate) fn parameter_sets(&self) -> Result<BTreeMap<u64, ParameterSet>> {
        let mut sets: BTreeMap<u64, ParameterSet> = BTreeMap::new();
        for cell in self.cells() {
            sets.entry(cell.run_id)
                .or_default()
                .insert(cell.name, cell.value)?;
        }
        Ok(sets)
    }

    pub(crate) fn to_msgpack_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        rmpv::encode::write_value(&mut bytes, &self.to_msgpack_value())
            .context("Failed to encode run-parameter table as MessagePack")?;
        Ok(bytes)
    }

    pub(crate) fn from_msgpack_bytes(bytes: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let value = rmpv::decode::read_value(&mut cursor)
            .context("Run-parameter table must be valid MessagePack")?;
        ensure!(
            cursor.position() == bytes.len() as u64,
            "Run-parameter table must contain exactly one MessagePack value",
        );
        Self::from_msgpack_value(value)
    }

    fn to_msgpack_value(&self) -> MessagePackValue {
        MessagePackValue::Map(vec![(
            MessagePackValue::from("columns"),
            MessagePackValue::Map(
                self.columns
                    .iter()
                    .map(|(name, column)| {
                        (
                            MessagePackValue::from(name.clone()),
                            column.to_msgpack_value(),
                        )
                    })
                    .collect(),
            ),
        )])
    }

    fn from_msgpack_value(value: MessagePackValue) -> Result<Self> {
        let mut fields = expect_msgpack_map(value, "run-parameter table")?;
        let columns = take_msgpack_field(&mut fields, "columns", "run-parameter table")?;
        reject_unknown_msgpack_fields(fields, "run-parameter table")?;

        let mut decoded_columns = BTreeMap::new();
        for (name, column) in expect_msgpack_map(columns, "run-parameter table columns")? {
            let name = expect_msgpack_string(name, "run-parameter column name")?;
            let column = RunParameterColumn::from_msgpack_value(column)
                .with_context(|| format!("Invalid run parameter column `{name}`"))?;
            if decoded_columns.insert(name.clone(), column).is_some() {
                crate::bail!("Duplicate run parameter column `{name}`");
            }
        }
        Ok(Self {
            columns: decoded_columns,
        })
    }
}

#[derive(Debug, Clone)]
enum RunParameterColumn {
    Bool(BTreeMap<u64, bool>),
    Int(BTreeMap<u64, i64>),
    Float(BTreeMap<u64, f64>),
    String(BTreeMap<u64, String>),
}

impl RunParameterColumn {
    fn to_msgpack_value(&self) -> MessagePackValue {
        let (type_name, values) = match self {
            Self::Bool(values) => (
                "bool",
                MessagePackValue::Map(
                    values
                        .iter()
                        .map(|(run_id, value)| {
                            (
                                MessagePackValue::from(*run_id),
                                MessagePackValue::from(*value),
                            )
                        })
                        .collect(),
                ),
            ),
            Self::Int(values) => (
                "int64",
                MessagePackValue::Map(
                    values
                        .iter()
                        .map(|(run_id, value)| {
                            (
                                MessagePackValue::from(*run_id),
                                MessagePackValue::from(*value),
                            )
                        })
                        .collect(),
                ),
            ),
            Self::Float(values) => (
                "float64",
                MessagePackValue::Map(
                    values
                        .iter()
                        .map(|(run_id, value)| {
                            (
                                MessagePackValue::from(*run_id),
                                MessagePackValue::from(*value),
                            )
                        })
                        .collect(),
                ),
            ),
            Self::String(values) => (
                "string",
                MessagePackValue::Map(
                    values
                        .iter()
                        .map(|(run_id, value)| {
                            (
                                MessagePackValue::from(*run_id),
                                MessagePackValue::from(value.clone()),
                            )
                        })
                        .collect(),
                ),
            ),
        };

        MessagePackValue::Map(vec![
            (
                MessagePackValue::from("type"),
                MessagePackValue::from(type_name),
            ),
            (MessagePackValue::from("values"), values),
        ])
    }

    fn from_msgpack_value(value: MessagePackValue) -> Result<Self> {
        let mut fields = expect_msgpack_map(value, "run-parameter column")?;
        let type_name = expect_msgpack_string(
            take_msgpack_field(&mut fields, "type", "run-parameter column")?,
            "run-parameter column type",
        )?;
        let values = take_msgpack_field(&mut fields, "values", "run-parameter column")?;
        reject_unknown_msgpack_fields(fields, "run-parameter column")?;

        match type_name.as_str() {
            "bool" => Ok(Self::Bool(decode_run_parameter_values(
                values,
                "bool",
                |value| {
                    value
                        .as_bool()
                        .ok_or_else(|| anyhow::anyhow!("Expected bool run parameter value"))
                },
            )?)),
            "int64" => Ok(Self::Int(decode_run_parameter_values(
                values,
                "int64",
                |value| {
                    value
                        .as_i64()
                        .ok_or_else(|| anyhow::anyhow!("Expected int64 run parameter value"))
                },
            )?)),
            "float64" => Ok(Self::Float(decode_run_parameter_values(
                values,
                "float64",
                |value| match value {
                    MessagePackValue::F64(value) => Ok(value),
                    _ => crate::bail!("Expected float64 run parameter value"),
                },
            )?)),
            "string" => Ok(Self::String(decode_run_parameter_values(
                values,
                "string",
                |value| expect_msgpack_string(value, "string run parameter value"),
            )?)),
            _ => crate::bail!("Unknown run parameter column type `{type_name}`"),
        }
    }

    fn cells(&self, name: &str) -> Vec<RunParameterCell> {
        match self {
            Self::Bool(values) => values
                .iter()
                .map(|(run_id, value)| RunParameterCell {
                    run_id: *run_id,
                    name: name.to_string(),
                    value: ParameterValue::Bool(*value),
                })
                .collect(),
            Self::Int(values) => values
                .iter()
                .map(|(run_id, value)| RunParameterCell {
                    run_id: *run_id,
                    name: name.to_string(),
                    value: ParameterValue::Int(*value),
                })
                .collect(),
            Self::Float(values) => values
                .iter()
                .map(|(run_id, value)| RunParameterCell {
                    run_id: *run_id,
                    name: name.to_string(),
                    value: ParameterValue::Float(*value),
                })
                .collect(),
            Self::String(values) => values
                .iter()
                .map(|(run_id, value)| RunParameterCell {
                    run_id: *run_id,
                    name: name.to_string(),
                    value: ParameterValue::String(value.clone()),
                })
                .collect(),
        }
    }

    fn from_value(value: &ParameterValue) -> Self {
        match value {
            ParameterValue::Bool(_) => Self::Bool(BTreeMap::new()),
            ParameterValue::Int(_) => Self::Int(BTreeMap::new()),
            ParameterValue::Float(_) => Self::Float(BTreeMap::new()),
            ParameterValue::String(_) => Self::String(BTreeMap::new()),
        }
    }

    fn insert(&mut self, name: &str, run_id: u64, value: &ParameterValue) -> Result<()> {
        match (self, value) {
            (Self::Bool(values), ParameterValue::Bool(value)) => {
                values.insert(run_id, *value);
                Ok(())
            }
            (Self::Int(values), ParameterValue::Int(value)) => {
                values.insert(run_id, *value);
                Ok(())
            }
            (column @ Self::Int(_), ParameterValue::Float(value)) => {
                let mut values = match std::mem::replace(column, Self::Float(BTreeMap::new())) {
                    Self::Int(values) => values
                        .into_iter()
                        .map(|(run_id, value)| (run_id, value as f64))
                        .collect::<BTreeMap<_, _>>(),
                    _ => unreachable!(),
                };
                values.insert(run_id, *value);
                *column = Self::Float(values);
                Ok(())
            }
            (Self::Float(values), ParameterValue::Int(value)) => {
                values.insert(run_id, *value as f64);
                Ok(())
            }
            (Self::Float(values), ParameterValue::Float(value)) => {
                values.insert(run_id, *value);
                Ok(())
            }
            (Self::String(values), ParameterValue::String(value)) => {
                values.insert(run_id, value.clone());
                Ok(())
            }
            (column, value) => {
                crate::bail!(
                    "Run parameter `{name}` has mixed column types: existing {}, incoming {}",
                    column.type_name(),
                    value.type_name()
                )
            }
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int64",
            Self::Float(_) => "float64",
            Self::String(_) => "string",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunParameterCell {
    pub run_id: u64,
    pub name: String,
    pub value: ParameterValue,
}

fn decode_run_parameter_values<T>(
    value: MessagePackValue,
    type_name: &str,
    decode_value: impl Fn(MessagePackValue) -> Result<T>,
) -> Result<BTreeMap<u64, T>> {
    let mut decoded = BTreeMap::new();
    for (run_id, value) in expect_msgpack_map(value, "run-parameter column values")? {
        let run_id = run_id.as_u64().ok_or_else(|| {
            anyhow::anyhow!("Run parameter `{type_name}` column run id must be uint64")
        })?;
        let value = decode_value(value).with_context(|| {
            format!("Invalid run parameter `{type_name}` value for run {run_id}")
        })?;
        if decoded.insert(run_id, value).is_some() {
            crate::bail!("Duplicate run parameter value for run {run_id}");
        }
    }
    Ok(decoded)
}

fn expect_msgpack_map(
    value: MessagePackValue,
    context: &str,
) -> Result<Vec<(MessagePackValue, MessagePackValue)>> {
    match value {
        MessagePackValue::Map(map) => Ok(map),
        _ => crate::bail!("{context} must be a MessagePack map"),
    }
}

fn expect_msgpack_string(value: MessagePackValue, context: &str) -> Result<String> {
    match value.as_str() {
        Some(value) => Ok(value.to_string()),
        None => crate::bail!("{context} must be a MessagePack string"),
    }
}

fn take_msgpack_field(
    fields: &mut Vec<(MessagePackValue, MessagePackValue)>,
    name: &str,
    context: &str,
) -> Result<MessagePackValue> {
    let mut found = None;
    let mut index = 0;
    while index < fields.len() {
        let field_name = match fields[index].0.as_str() {
            Some(field_name) => field_name.to_string(),
            None => crate::bail!("{context} field name must be a MessagePack string"),
        };
        if field_name == name {
            let (_, value) = fields.swap_remove(index);
            if found.replace(value).is_some() {
                crate::bail!("{context} contains duplicate `{name}` field");
            }
        } else {
            index += 1;
        }
    }
    found.ok_or_else(|| anyhow::anyhow!("{context} is missing `{name}` field"))
}

fn reject_unknown_msgpack_fields(
    fields: Vec<(MessagePackValue, MessagePackValue)>,
    context: &str,
) -> Result<()> {
    if let Some((name, _)) = fields.into_iter().next() {
        let name = expect_msgpack_string(name, "unknown MessagePack field name")?;
        crate::bail!("{context} contains unknown `{name}` field");
    }
    Ok(())
}
