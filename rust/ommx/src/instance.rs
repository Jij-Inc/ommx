use crate::{
    error::ParseError, v1, Constraint, ConstraintID, DecisionVariable, Function, RemovedConstraint,
    VariableID,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sense {
    Minimize,
    Maximize,
}

impl TryFrom<v1::instance::Sense> for Sense {
    type Error = ParseError;
    fn try_from(value: v1::instance::Sense) -> Result<Self, Self::Error> {
        match value {
            v1::instance::Sense::Minimize => Ok(Self::Minimize),
            v1::instance::Sense::Maximize => Ok(Self::Maximize),
            v1::instance::Sense::Unspecified => Err(ParseError::UnspecifiedEnum {
                enum_name: "ommx.v1.instance.Sense",
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    sense: Sense,
    objective: Function,
    decision_variables: HashMap<VariableID, DecisionVariable>,
    constraints: HashMap<ConstraintID, Constraint>,
    removed_constraints: HashMap<ConstraintID, RemovedConstraint>,
}

impl TryFrom<v1::Instance> for Instance {
    type Error = ParseError;
    fn try_from(value: v1::Instance) -> Result<Self, Self::Error> {
        let sense = value.sense().try_into()?;

        let objective = value
            .objective
            .ok_or(ParseError::MissingField {
                message: "ommx.v1.Instance",
                field: "objective",
            })?
            .try_into()?;

        let mut decision_variables = HashMap::new();
        for v in value.decision_variables {
            let v: DecisionVariable = v.try_into()?;
            let id = v.id;
            if decision_variables.insert(id, v).is_some() {
                return Err(ParseError::DuplicatedVariableID { id });
            }
        }

        let mut constraints = HashMap::new();
        for c in value.constraints {
            let c: Constraint = c.try_into()?;
            let id = c.id;
            if constraints.insert(id, c).is_some() {
                return Err(ParseError::DuplicatedConstraintID { id });
            }
        }

        let mut removed_constraints = HashMap::new();
        for c in value.removed_constraints {
            let c: RemovedConstraint = c.try_into()?;
            let id = c.constraint.id;
            if constraints.contains_key(&id) {
                return Err(ParseError::DuplicatedConstraintID { id });
            }
            if removed_constraints.insert(id, c).is_some() {
                return Err(ParseError::DuplicatedConstraintID { id });
            }
        }

        Ok(Self {
            sense,
            objective,
            constraints,
            decision_variables,
            removed_constraints,
        })
    }
}
