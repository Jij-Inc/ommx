use crate::{
    parse::{as_constraint_id, as_variable_id, Parse, ParseError, RawParseError},
    v1::{self, State},
    Constraint, ConstraintID, ConstraintIDSet, DecisionVariable, InstanceError, RemovedConstraint,
    VariableID, VariableIDSet,
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

impl OneHot {
    /// Apply partial evaluation to this OneHot constraint hint.
    /// Returns Some(updated_hint) if the hint should be kept, None if it should be discarded.
    pub fn partial_evaluate(mut self, state: &State, atol: crate::ATol) -> Option<Self> {
        let mut variables_to_remove = Vec::new();

        for &var_id in &self.variables {
            if let Some(&value) = state.entries.get(&var_id.into_inner()) {
                if value.abs() < *atol {
                    // If the value is 0 (within tolerance), remove the variable
                    variables_to_remove.push(var_id);
                } else {
                    // If the value is non-zero, warn and discard the hint
                    log::warn!(
                        "OneHot constraint hint {} has variable {} with non-zero value {}. Discarding the hint.",
                        self.id,
                        var_id,
                        value
                    );
                    return None; // Discard the hint
                }
            }
        }

        // Remove variables with zero values
        for var in variables_to_remove {
            self.variables.remove(&var);
        }

        Some(self) // Keep the updated hint
    }

    /// Get all decision variable IDs used by this constraint hint
    pub fn used_decision_variable_ids(&self) -> VariableIDSet {
        self.variables.clone()
    }

    /// Get all constraint IDs used by this constraint hint
    pub fn used_constraint_ids(&self) -> ConstraintIDSet {
        [self.id].into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::State;
    use maplit::{btreeset, hashmap};

    #[test]
    fn test_one_hot_partial_evaluate_remove_zero() {
        // Test that OneHot removes variables with value 0
        let one_hot = OneHot {
            id: ConstraintID::from(1),
            variables: btreeset! {
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            },
        };

        let state = State {
            entries: hashmap! {
                1 => 0.0,  // Should be removed
                2 => 0.0,  // Should be removed
            },
        };

        let result = one_hot.partial_evaluate(&state, crate::ATol::default());

        // Should keep the hint and only variable 3 should remain
        assert!(result.is_some());
        let updated_hint = result.unwrap();
        assert_eq!(updated_hint.variables.len(), 1);
        assert!(updated_hint.variables.contains(&VariableID::from(3)));
    }

    #[test]
    fn test_one_hot_partial_evaluate_discard_nonzero() {
        // Test that OneHot is discarded when a variable has non-zero value
        let one_hot = OneHot {
            id: ConstraintID::from(1),
            variables: btreeset! {
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            },
        };

        let state = State {
            entries: hashmap! {
                1 => 0.0,
                2 => 1.0,  // Non-zero value should cause discard
            },
        };

        let result = one_hot.partial_evaluate(&state, crate::ATol::default());

        // Should discard the hint
        assert!(result.is_none());
    }
}
