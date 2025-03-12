use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, Function,
};
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
            _ => Err(RawParseError::UnspecifiedEnum {
                enum_name: "ommx.v1.Equality",
            }
            .into()),
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
            equality: value.equality().parse("ommx.v1.Constraint", "equality")?,
            function: value
                .function
                .ok_or(RawParseError::MissingField {
                    message: "ommx.v1.Constraint",
                    field: "function",
                })?
                .parse("ommx.v1.Constraint", "function")?,
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
                .ok_or(RawParseError::MissingField {
                    message: "ommx.v1.RemovedConstraint",
                    field: "constraint",
                })?
                .parse("ommx.v1.RemovedConstraint", "constraint")?,
            removed_reason: value.removed_reason,
            removed_reason_parameters: value.removed_reason_parameters,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_message() {
        let out: Result<RemovedConstraint, ParseError> = v1::RemovedConstraint {
            constraint: Some(v1::Constraint {
                id: 1,
                function: Some(v1::Function { function: None }),
                equality: v1::Equality::EqualToZero as i32,
                ..Default::default()
            }),
            removed_reason: "reason".to_string(),
            removed_reason_parameters: Default::default(),
        }
        .try_into();

        insta::assert_snapshot!(out.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.RemovedConstraint[constraint]
          └─ommx.v1.Constraint[function]
        Unsupported ommx.v1.Function is found. It is created by a newer version of OMMX SDK.
        "###);
    }
}
