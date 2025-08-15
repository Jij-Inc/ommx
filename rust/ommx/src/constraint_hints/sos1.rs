use crate::{
    parse::{as_constraint_id, as_variable_id, Parse, ParseError, RawParseError},
    v1::{self},
    Constraint, ConstraintID, DecisionVariable, InstanceError, RemovedConstraint, VariableID,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sos1 {
    pub binary_constraint_id: ConstraintID,
    pub big_m_constraint_ids: BTreeSet<ConstraintID>,
    pub variables: BTreeSet<VariableID>,
}

impl Parse for v1::Sos1 {
    type Output = Sos1;
    type Context = (
        BTreeMap<VariableID, DecisionVariable>,
        BTreeMap<ConstraintID, Constraint>,
        BTreeMap<ConstraintID, RemovedConstraint>,
    );
    fn parse(
        self,
        (decision_variable, constraints, removed_constraints): &Self::Context,
    ) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Sos1";
        let binary_constraint_id =
            as_constraint_id(constraints, removed_constraints, self.binary_constraint_id)
                .map_err(|e| e.context(message, "binary_constraint_id"))?;
        let mut big_m_constraint_ids = BTreeSet::new();
        for id in &self.big_m_constraint_ids {
            let id = as_constraint_id(constraints, removed_constraints, *id)
                .map_err(|e| e.context(message, "big_m_constraint_ids"))?;
            if !big_m_constraint_ids.insert(id) {
                return Err(
                    RawParseError::InstanceError(InstanceError::NonUniqueConstraintID { id })
                        .context(message, "big_m_constraint_ids"),
                );
            }
        }
        let mut variables = BTreeSet::new();
        for id in &self.decision_variables {
            let id = as_variable_id(decision_variable, *id)
                .map_err(|e| e.context(message, "decision_variables"))?;
            if !variables.insert(id) {
                return Err(
                    RawParseError::InstanceError(InstanceError::NonUniqueVariableID { id })
                        .context(message, "decision_variables"),
                );
            }
        }
        Ok(Sos1 {
            binary_constraint_id,
            big_m_constraint_ids,
            variables,
        })
    }
}

impl From<Sos1> for v1::Sos1 {
    fn from(value: Sos1) -> Self {
        Self {
            binary_constraint_id: *value.binary_constraint_id,
            big_m_constraint_ids: value.big_m_constraint_ids.into_iter().map(|c| *c).collect(),
            decision_variables: value.variables.into_iter().map(|v| *v).collect(),
        }
    }
}
