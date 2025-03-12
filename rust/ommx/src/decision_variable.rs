use crate::{parse::*, v1};
use derive_more::{Deref, From};
use std::collections::HashMap;

/// ID for decision variable and parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct VariableID(u64);

#[derive(Debug, Clone, PartialEq)]
pub struct Bound {
    pub lower: f64,
    pub upper: f64,
}

impl Default for Bound {
    fn default() -> Self {
        Self {
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
        }
    }
}

impl From<v1::Bound> for Bound {
    fn from(bound: v1::Bound) -> Self {
        Self {
            lower: bound.lower,
            upper: bound.upper,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Continuous,
    Integer,
    Binary,
    SemiContinuous,
    SemiInteger,
}

impl TryFrom<v1::decision_variable::Kind> for Kind {
    type Error = ParseError;

    fn try_from(value: v1::decision_variable::Kind) -> Result<Self, Self::Error> {
        use v1::decision_variable::Kind::*;
        match value {
            Unspecified => Err(crate::parse::RawParseError::UnspecifiedEnum {
                enum_name: "ommx.v1.decision_variable.Kind",
            }
            .into()),
            Continuous => Ok(Self::Continuous),
            Integer => Ok(Self::Integer),
            Binary => Ok(Self::Binary),
            SemiContinuous => Ok(Self::SemiContinuous),
            SemiInteger => Ok(Self::SemiInteger),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionVariable {
    pub id: VariableID,
    pub kind: Kind,
    pub bound: Bound,

    pub substituted_value: Option<f64>,

    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: HashMap<String, String>,
    pub description: Option<String>,
}

impl TryFrom<v1::DecisionVariable> for DecisionVariable {
    type Error = ParseError;

    fn try_from(value: v1::DecisionVariable) -> Result<Self, Self::Error> {
        Ok(Self {
            id: VariableID(value.id),
            kind: value.kind().try_into()?,
            bound: value.bound.map(Bound::from).unwrap_or_default(),
            substituted_value: value.substituted_value,
            name: value.name,
            subscripts: value.subscripts,
            parameters: value.parameters,
            description: value.description,
        })
    }
}
