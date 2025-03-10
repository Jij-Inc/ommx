use crate::{error::ParseError, v1, Constraint, ConstraintID, ConstraintMetadata, Function};
use std::{collections::HashMap, hash::Hash};

/// ID for decision variable
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariableID(u64);

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    objective: Function,
    constraints: HashMap<ConstraintID, Constraint>,
    constraint_metadata: HashMap<ConstraintID, ConstraintMetadata>,
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
        let mut constraints = HashMap::new();
        let mut constraint_metadata = HashMap::new();
        for c in value.constraints {
            let (id, c, metadata) = c.try_into()?;
            if constraints.insert(id, c).is_some() {
                return Err(ParseError::DuplicatedConstraintID { id });
            }
            constraint_metadata.insert(id, metadata);
        }
        Ok(Self {
            objective,
            constraints,
            constraint_metadata,
        })
    }
}
