use crate::{
    parse::{as_constraint_id, as_variable_id, Parse, ParseError, RawParseError},
    v1::{self},
    Constraint, ConstraintID, DecisionVariable, InstanceError, RemovedConstraint, VariableID,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneHot {
    pub id: ConstraintID,
    pub variables: BTreeSet<VariableID>,
}

impl Parse for v1::OneHot {
    type Output = OneHot;
    type Context = (
        BTreeMap<VariableID, DecisionVariable>,
        BTreeMap<ConstraintID, Constraint>,
        BTreeMap<ConstraintID, RemovedConstraint>,
    );
    fn parse(
        self,
        (decision_variable, constraints, removed_constraints): &Self::Context,
    ) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.OneHot";
        let constraint_id = as_constraint_id(constraints, removed_constraints, self.constraint_id)
            .map_err(|e| e.context(message, "constraint_id"))?;
        let mut variables = BTreeSet::new();
        for v in &self.decision_variables {
            let id = as_variable_id(decision_variable, *v)
                .map_err(|e| e.context(message, "decision_variables"))?;
            if !variables.insert(id) {
                return Err(
                    RawParseError::InstanceError(InstanceError::NonUniqueVariableID { id })
                        .context(message, "decision_variables"),
                );
            }
        }
        Ok(OneHot {
            id: constraint_id,
            variables,
        })
    }
}

impl From<OneHot> for v1::OneHot {
    fn from(value: OneHot) -> Self {
        Self {
            constraint_id: *value.id,
            decision_variables: value.variables.into_iter().map(|v| *v).collect(),
        }
    }
}
