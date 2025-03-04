use crate::{error::ParseError, v1, Function};
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
                enum_name: "Equality",
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Constraint<F: Function> {
    // Required
    pub id: u64,
    pub function: F,
    pub equality: Equality,

    // Metadata
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: HashMap<String, String>,
    pub description: Option<String>,
}

impl<F: Function> Constraint<F> {
    pub fn new(id: u64, function: F, equality: Equality) -> Self {
        Self {
            id,
            function,
            equality,
            name: None,
            subscripts: Vec::new(),
            parameters: HashMap::new(),
            description: None,
        }
    }
}

impl<F> TryFrom<v1::Constraint> for Constraint<F>
where
    F: Function + TryFrom<v1::Function, Error = ParseError>,
{
    type Error = ParseError;
    fn try_from(value: v1::Constraint) -> Result<Self, Self::Error> {
        let equality = value.equality().try_into()?;
        let function = value.function.ok_or(ParseError::UnsupportedV1Function)?;
        Ok(Self {
            id: value.id,
            function: function.try_into()?,
            equality,
            name: value.name,
            subscripts: value.subscripts,
            parameters: value.parameters,
            description: value.description,
        })
    }
}
