use crate::{error::ParseError, v1, Constraint, ConstraintID, Function};
use std::{collections::HashMap, hash::Hash};

/// ID for decision variable
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariableID(u64);

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    objective: Function,
    constraints: HashMap<ConstraintID, Constraint>,
}

impl TryFrom<v1::Instance> for Instance {
    type Error = ParseError;
    fn try_from(value: v1::Instance) -> Result<Self, Self::Error> {
        let objective = value
            .objective
            .ok_or(ParseError::MissingField {
                message: "ommx.v1.Instance",
                field: "objective",
            })?
            .try_into()?;
        let constraints = value
            .constraints
            .into_iter()
            .map(|c| Ok((c.id.into(), c.try_into()?)))
            .collect::<Result<_, ParseError>>()?;
        Ok(Self {
            objective,
            constraints,
        })
    }
}
