use crate::{
    error::ParseError, v1, Constraint, ConstraintID, DecisionVariable, Function, VariableID,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    objective: Function,
    constraints: HashMap<ConstraintID, Constraint>,
    decision_variables: HashMap<VariableID, DecisionVariable>,
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
        for c in value.constraints {
            let c: Constraint = c.try_into()?;
            let id = c.id;
            if constraints.insert(id, c).is_some() {
                return Err(ParseError::DuplicatedConstraintID { id });
            }
        }

        let mut decision_variables = HashMap::new();
        for v in value.decision_variables {
            let v: DecisionVariable = v.try_into()?;
            let id = v.id;
            if decision_variables.insert(id, v).is_some() {
                return Err(ParseError::DuplicatedVariableID { id });
            }
        }

        Ok(Self {
            objective,
            constraints,
            decision_variables,
        })
    }
}
