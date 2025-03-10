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

#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub function: Function,
    pub equality: Equality,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConstraintMetadata {
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: HashMap<String, String>,
    pub description: Option<String>,
}

impl TryFrom<v1::Constraint> for (ConstraintID, Constraint, ConstraintMetadata) {
    type Error = ParseError;
    fn try_from(value: v1::Constraint) -> Result<Self, Self::Error> {
        let equality = value.equality().try_into()?;
        let function = value.function.ok_or(ParseError::UnsupportedV1Function)?;
        let id = ConstraintID(value.id);
        let constraint = Constraint {
            function: function.try_into()?,
            equality,
        };
        let metadata = ConstraintMetadata {
            name: value.name,
            subscripts: value.subscripts,
            parameters: value.parameters,
            description: value.description,
        };
        Ok((id, constraint, metadata))
    }
}
