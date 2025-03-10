use crate::{error::ParseError, v1, Function};
use derive_more::{Deref, From};
use std::collections::HashMap;

/// Constraint equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Equality {
    /// $f(x) = 0$ type constraint.
    EqualToZero,
    /// $f(x) \leq 0$ type constraint.
    LessThanOrEqualToZero,
}

impl TryFrom<v1::Equality> for Equality {
    type Error = ParseError;
    fn try_from(value: v1::Equality) -> Result<Self, Self::Error> {
        match value {
            v1::Equality::EqualToZero => Ok(Self::EqualToZero),
            v1::Equality::LessThanOrEqualToZero => Ok(Self::LessThanOrEqualToZero),
            _ => Err(ParseError::UnspecifiedEnum {
                enum_name: "ommx.v1.Equality",
            }),
        }
    }
}

/// ID for constraint
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct ConstraintID(u64);

/// `ommx.v1.Constraint` with validated, typed fields.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub id: ConstraintID,
    pub function: Function,
    pub equality: Equality,
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: HashMap<String, String>,
    pub description: Option<String>,
}

impl TryFrom<v1::Constraint> for Constraint {
    type Error = ParseError;
    fn try_from(value: v1::Constraint) -> Result<Self, Self::Error> {
        Ok(Self {
            id: ConstraintID(value.id),
            equality: value.equality().try_into()?,
            function: value
                .function
                .ok_or(ParseError::MissingField {
                    message: "ommx.v1.Constraint",
                    field: "function",
                })?
                .try_into()?,
            name: value.name,
            subscripts: value.subscripts,
            parameters: value.parameters,
            description: value.description,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemovedConstraint {
    pub constraint: Constraint,
    pub removed_reason: String,
    pub removed_reason_parameters: HashMap<String, String>,
}

impl TryFrom<v1::RemovedConstraint> for RemovedConstraint {
    type Error = ParseError;
    fn try_from(value: v1::RemovedConstraint) -> Result<Self, Self::Error> {
        Ok(Self {
            constraint: value
                .constraint
                .ok_or(ParseError::MissingField {
                    message: "ommx.v1.RemovedConstraint",
                    field: "constraint",
                })?
                .try_into()?,
            removed_reason: value.removed_reason,
            removed_reason_parameters: value.removed_reason_parameters,
        })
    }
}
