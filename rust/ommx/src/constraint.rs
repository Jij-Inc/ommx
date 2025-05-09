use crate::{
    parse::{Parse, ParseError, RawParseError},
    v1, Function,
};
use approx::AbsDiffEq;
use derive_more::{Deref, From};
use fnv::FnvHashMap;

/// Constraint equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Equality {
    /// $f(x) = 0$ type constraint.
    EqualToZero,
    /// $f(x) \leq 0$ type constraint.
    LessThanOrEqualToZero,
}

impl Parse for v1::Equality {
    type Output = Equality;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        match self {
            v1::Equality::EqualToZero => Ok(Equality::EqualToZero),
            v1::Equality::LessThanOrEqualToZero => Ok(Equality::LessThanOrEqualToZero),
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
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}

impl AbsDiffEq for Constraint {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        Function::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        self.equality == other.equality && self.function.abs_diff_eq(&other.function, epsilon)
    }
}

impl Parse for v1::Constraint {
    type Output = Constraint;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Constraint";
        Ok(Constraint {
            id: ConstraintID(self.id),
            equality: self.equality().parse_as(&(), message, "equality")?,
            function: self
                .function
                .ok_or(RawParseError::MissingField {
                    message,
                    field: "function",
                })?
                .parse_as(&(), message, "function")?,
            name: self.name,
            subscripts: self.subscripts,
            parameters: self.parameters.into_iter().collect(),
            description: self.description,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemovedConstraint {
    pub constraint: Constraint,
    pub removed_reason: String,
    pub removed_reason_parameters: FnvHashMap<String, String>,
}

impl AbsDiffEq for RemovedConstraint {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        Constraint::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        self.constraint.abs_diff_eq(&other.constraint, epsilon)
    }
}

impl Parse for v1::RemovedConstraint {
    type Output = RemovedConstraint;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.RemovedConstraint";
        Ok(RemovedConstraint {
            constraint: self
                .constraint
                .ok_or(RawParseError::MissingField {
                    message,
                    field: "constraint",
                })?
                .parse_as(&(), message, "constraint")?,
            removed_reason: self.removed_reason,
            removed_reason_parameters: self.removed_reason_parameters.into_iter().collect(),
        })
    }
}

impl Parse for Vec<v1::Constraint> {
    type Output = FnvHashMap<ConstraintID, Constraint>;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut constraints = FnvHashMap::default();
        for c in self {
            let c: Constraint = c.parse(&())?;
            let id = c.id;
            if constraints.insert(id, c).is_some() {
                return Err(RawParseError::DuplicatedConstraintID { id }.into());
            }
        }
        Ok(constraints)
    }
}

impl Parse for Vec<v1::RemovedConstraint> {
    type Output = FnvHashMap<ConstraintID, RemovedConstraint>;
    type Context = FnvHashMap<ConstraintID, Constraint>;
    fn parse(self, constraints: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut removed_constraints = FnvHashMap::default();
        for c in self {
            let c: RemovedConstraint = c.parse(&())?;
            let id = c.constraint.id;
            if constraints.contains_key(&id) {
                return Err(RawParseError::DuplicatedConstraintID { id }.into());
            }
            if removed_constraints.insert(id, c).is_some() {
                return Err(RawParseError::DuplicatedConstraintID { id }.into());
            }
        }
        Ok(removed_constraints)
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
        .parse(&());

        insta::assert_snapshot!(out.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.RemovedConstraint[constraint]
          └─ommx.v1.Constraint[function]
        Unsupported ommx.v1.Function is found. It is created by a newer version of OMMX SDK.
        "###);
    }
}
