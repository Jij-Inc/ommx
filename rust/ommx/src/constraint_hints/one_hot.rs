use crate::{
    parse::{as_constraint_id, as_variable_id, Parse, ParseError, RawParseError},
    v1::{self, State},
    ATol, Constraint, ConstraintID, DecisionVariable, InstanceError, RemovedConstraint, VariableID,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneHot {
    pub id: ConstraintID,
    pub variables: BTreeSet<VariableID>,
}

impl OneHot {
    /// Partially evaluate the OneHot constraint with given state.
    ///
    /// - If a decision variable is assigned 0, it is removed from the constraint
    /// - If a decision variable is fixed to a non-zero value, the entire hint is discarded with a warning
    ///
    /// Returns `None` if the constraint should be discarded, otherwise returns the updated constraint.
    pub fn partial_evaluate(mut self, state: &State, atol: ATol) -> Option<Self> {
        // Check each variable in the state
        for (&var_u64, &value) in &state.entries {
            let var_id = VariableID::from(var_u64);

            // Only process if this variable is in our constraint
            if self.variables.contains(&var_id) {
                // Check if the value is approximately zero using ATol
                if value.abs() < atol {
                    // Variable is 0, remove it from the constraint
                    self.variables.remove(&var_id);
                } else {
                    // Variable is fixed to non-zero value, discard the entire hint
                    log::warn!(
                        "OneHot constraint {:?}: variable {:?} is fixed to non-zero value {}, discarding the entire hint",
                        self.id,
                        var_id,
                        value
                    );
                    return None;
                }
            }
        }

        Some(self)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partial_evaluate_removes_zero_variables() {
        // Create a OneHot constraint with variables 1, 2, 3
        let one_hot = OneHot {
            id: ConstraintID::from(100),
            variables: vec![
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            ]
            .into_iter()
            .collect(),
        };

        // Create a state where variable 2 is set to 0
        let mut state = State::default();
        state.entries.insert(2, 0.0);

        // Apply partial evaluation
        let atol = ATol::new(1e-10).unwrap();
        let result = one_hot.partial_evaluate(&state, atol).unwrap();

        // Check that variable 2 was removed
        assert_eq!(result.variables.len(), 2);
        assert!(result.variables.contains(&VariableID::from(1)));
        assert!(!result.variables.contains(&VariableID::from(2)));
        assert!(result.variables.contains(&VariableID::from(3)));
    }

    #[test]
    fn test_partial_evaluate_discards_on_nonzero_fixed() {
        // Create a OneHot constraint with variables 1, 2, 3
        let one_hot = OneHot {
            id: ConstraintID::from(100),
            variables: vec![
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            ]
            .into_iter()
            .collect(),
        };

        // Create a state where variable 2 is set to 1.0 (non-zero)
        let mut state = State::default();
        state.entries.insert(2, 1.0);

        // Apply partial evaluation
        let atol = ATol::new(1e-10).unwrap();
        let result = one_hot.partial_evaluate(&state, atol);

        // Check that the constraint was discarded
        assert!(result.is_none());
    }

    #[test]
    fn test_partial_evaluate_with_atol() {
        // Create a OneHot constraint with variables 1, 2, 3
        let one_hot = OneHot {
            id: ConstraintID::from(100),
            variables: vec![
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            ]
            .into_iter()
            .collect(),
        };

        // Create a state where variable 2 is set to a very small value
        let mut state = State::default();
        state.entries.insert(2, 1e-11);

        // Apply partial evaluation with atol = 1e-10
        let atol = ATol::new(1e-10).unwrap();
        let result = one_hot.partial_evaluate(&state, atol).unwrap();

        // Check that variable 2 was removed (since 1e-11 < 1e-10)
        assert_eq!(result.variables.len(), 2);
        assert!(!result.variables.contains(&VariableID::from(2)));
    }

    #[test]
    fn test_partial_evaluate_empty_state() {
        // Create a OneHot constraint with variables 1, 2, 3
        let one_hot = OneHot {
            id: ConstraintID::from(100),
            variables: vec![
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            ]
            .into_iter()
            .collect(),
        };

        // Create an empty state
        let state = State::default();

        // Save the expected variables before moving one_hot
        let expected_variables = one_hot.variables.clone();

        // Apply partial evaluation
        let atol = ATol::new(1e-10).unwrap();
        let result = one_hot.partial_evaluate(&state, atol).unwrap();

        // Check that all variables remain
        assert_eq!(result.variables.len(), 3);
        assert_eq!(result.variables, expected_variables);
    }
}
