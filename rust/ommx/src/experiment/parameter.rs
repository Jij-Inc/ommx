//! Run parameter values and committed run-parameter table serialization.

use super::RunEntry;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{btree_map::Values, BTreeMap};

/// A scalar cell value accepted by the run parameter table.
///
/// Parameters are intended to become DataFrame / Arrow-like columns at
/// commit time, so this intentionally excludes nulls and structured
/// JSON values. Missing cells are represented by the absence of a
/// `(run_id, value)` entry in the committed column.
#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunParameterTable {
    columns: BTreeMap<String, RunParameterColumn>,
}

impl RunParameterTable {
    pub fn from_runs<'reg>(runs: Values<'_, u64, RunEntry<'reg>>) -> Result<Self> {
        let mut columns = BTreeMap::new();
        for run in runs {
            for (name, value) in &run.parameters {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "values")]
enum RunParameterColumn {
    #[serde(rename = "bool")]
    Bool(BTreeMap<u64, bool>),
    #[serde(rename = "int64")]
    Int(BTreeMap<u64, i64>),
    #[serde(rename = "float64")]
    Float(BTreeMap<u64, f64>),
    #[serde(rename = "string")]
    String(BTreeMap<u64, String>),
}

impl RunParameterColumn {
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
