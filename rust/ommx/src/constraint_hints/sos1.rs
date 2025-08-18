use crate::{
    parse::{as_constraint_id, as_variable_id, Parse, ParseError, RawParseError},
    v1::{self, State},
    ATol, Constraint, ConstraintID, DecisionVariable, InstanceError, RemovedConstraint, VariableID,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Result of partial evaluation for SOS1 constraint
#[derive(Debug, Clone, PartialEq)]
pub enum Sos1PartialEvaluateResult {
    /// Constraint was updated by removing zero variables
    Updated(Sos1),
    /// A variable was fixed to non-zero, so the constraint is satisfied
    /// Returns a State with variables to be fixed to 0
    AdditionalFix(State),
}

/// Error that can occur during partial evaluation of SOS1 constraint
#[derive(Debug, Clone, Error)]
pub enum Sos1PartialEvaluateError {
    #[error("Multiple variables are fixed to non-zero values in SOS1 constraint (binary: {binary_constraint_id:?}): {variables:?}")]
    MultipleNonZeroFixed {
        binary_constraint_id: ConstraintID,
        variables: Vec<(VariableID, f64)>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sos1 {
    pub binary_constraint_id: ConstraintID,
    pub big_m_constraint_ids: BTreeSet<ConstraintID>,
    pub variables: BTreeSet<VariableID>,
}

impl Sos1 {
    /// Partially evaluate the SOS1 constraint with given state.
    ///
    /// - If a decision variable is assigned 0, it is removed from the constraint
    /// - If exactly one variable is fixed to non-zero, other variables are fixed to 0
    /// - If multiple variables are fixed to non-zero values, returns an error
    /// - SOS1 allows all variables to be 0 (unlike OneHot)
    ///
    /// Returns a result indicating whether the constraint was updated, requires additional fixes, or has an error.
    pub fn partial_evaluate(
        mut self,
        state: &State,
        atol: ATol,
    ) -> Result<Sos1PartialEvaluateResult, Sos1PartialEvaluateError> {
        let mut fixed_to_nonzero: Option<VariableID> = None;
        let mut variables_to_remove = Vec::new();

        // Check each variable in the SOS1 constraint
        for &var_id in &self.variables {
            // Skip if variable is not in state
            let Some(&value) = state.entries.get(&(*var_id)) else {
                continue;
            };

            // Variable is approximately zero
            if value.abs() < atol {
                variables_to_remove.push(var_id);
                continue;
            }

            // Variable is non-zero
            if let Some(first_var) = fixed_to_nonzero {
                // Multiple variables fixed to non-zero - this violates SOS1
                return Err(Sos1PartialEvaluateError::MultipleNonZeroFixed {
                    binary_constraint_id: self.binary_constraint_id,
                    variables: vec![(first_var, 0.0), (var_id, value)], // We don't store the first value, use 0.0 placeholder
                });
            }
            fixed_to_nonzero = Some(var_id);
            variables_to_remove.push(var_id);
        }

        // Remove variables that are fixed
        for var_id in variables_to_remove {
            self.variables.remove(&var_id);
        }

        // Handle the different cases
        if fixed_to_nonzero.is_some() {
            // One variable is fixed to non-zero, need to fix remaining variables to 0
            let mut additional_fixes = State::default();
            for &var_id in &self.variables {
                additional_fixes.entries.insert(*var_id, 0.0);
            }
            Ok(Sos1PartialEvaluateResult::AdditionalFix(additional_fixes))
        } else {
            // No variable fixed to non-zero (all zeros or some unfixed)
            // For SOS1, this is valid - return the updated constraint
            Ok(Sos1PartialEvaluateResult::Updated(self))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partial_evaluate_removes_zero_variables() {
        // Create a SOS1 constraint with variables 1, 2, 3
        let sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(100),
            big_m_constraint_ids: vec![
                ConstraintID::from(101),
                ConstraintID::from(102),
                ConstraintID::from(103),
            ]
            .into_iter()
            .collect(),
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
        let result = sos1.partial_evaluate(&state, atol).unwrap();

        // Check that variable 2 was removed
        match result {
            Sos1PartialEvaluateResult::Updated(updated) => {
                assert_eq!(updated.variables.len(), 2);
                assert!(updated.variables.contains(&VariableID::from(1)));
                assert!(!updated.variables.contains(&VariableID::from(2)));
                assert!(updated.variables.contains(&VariableID::from(3)));
                // Check that constraint IDs are unchanged
                assert_eq!(updated.binary_constraint_id, ConstraintID::from(100));
                assert_eq!(updated.big_m_constraint_ids.len(), 3);
            }
            _ => panic!("Expected Updated result"),
        }
    }

    #[test]
    fn test_partial_evaluate_fixes_others_when_one_is_nonzero() {
        // Create a SOS1 constraint with variables 1, 2, 3
        let sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(100),
            big_m_constraint_ids: BTreeSet::new(),
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
        let result = sos1.partial_evaluate(&state, atol).unwrap();

        // Check that we get additional fixes for other variables
        match result {
            Sos1PartialEvaluateResult::AdditionalFix(fixes) => {
                assert_eq!(fixes.entries.len(), 2); // Two variables to fix
                assert_eq!(fixes.entries.get(&1), Some(&0.0));
                assert_eq!(fixes.entries.get(&3), Some(&0.0));
            }
            _ => panic!("Expected AdditionalFix result"),
        }
    }

    #[test]
    fn test_partial_evaluate_error_on_multiple_nonzero() {
        // Create a SOS1 constraint with variables 1, 2, 3
        let sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(100),
            big_m_constraint_ids: BTreeSet::new(),
            variables: vec![
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            ]
            .into_iter()
            .collect(),
        };

        // Create a state where variables 1 and 2 are both set to non-zero
        let mut state = State::default();
        state.entries.insert(1, 1.0);
        state.entries.insert(2, 2.0);

        // Apply partial evaluation
        let atol = ATol::new(1e-10).unwrap();
        let result = sos1.partial_evaluate(&state, atol);

        // Check that we get an error
        match result {
            Err(Sos1PartialEvaluateError::MultipleNonZeroFixed { variables, .. }) => {
                assert_eq!(variables.len(), 2);
            }
            _ => panic!("Expected MultipleNonZeroFixed error"),
        }
    }

    #[test]
    fn test_partial_evaluate_with_atol() {
        // Create a SOS1 constraint with variables 1, 2, 3
        let sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(100),
            big_m_constraint_ids: BTreeSet::new(),
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
        let result = sos1.partial_evaluate(&state, atol).unwrap();

        // Check that variable 2 was removed (since 1e-11 < 1e-10)
        match result {
            Sos1PartialEvaluateResult::Updated(updated) => {
                assert_eq!(updated.variables.len(), 2);
                assert!(!updated.variables.contains(&VariableID::from(2)));
            }
            _ => panic!("Expected Updated result"),
        }
    }

    #[test]
    fn test_partial_evaluate_empty_state() {
        // Create a SOS1 constraint with variables and big-M constraints
        let sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(100),
            big_m_constraint_ids: vec![ConstraintID::from(101), ConstraintID::from(102)]
                .into_iter()
                .collect(),
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

        // Save the expected values before moving sos1
        let expected_variables = sos1.variables.clone();
        let expected_binary_id = sos1.binary_constraint_id;
        let expected_big_m_ids = sos1.big_m_constraint_ids.clone();

        // Apply partial evaluation
        let atol = ATol::new(1e-10).unwrap();
        let result = sos1.partial_evaluate(&state, atol).unwrap();

        // Check that all fields remain unchanged
        match result {
            Sos1PartialEvaluateResult::Updated(updated) => {
                assert_eq!(updated.variables, expected_variables);
                assert_eq!(updated.binary_constraint_id, expected_binary_id);
                assert_eq!(updated.big_m_constraint_ids, expected_big_m_ids);
            }
            _ => panic!("Expected Updated result"),
        }
    }

    #[test]
    fn test_partial_evaluate_all_zeros_valid() {
        // Create a SOS1 constraint with variables 1, 2, 3
        let sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(100),
            big_m_constraint_ids: BTreeSet::new(),
            variables: vec![
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            ]
            .into_iter()
            .collect(),
        };

        // Create a state where all variables are set to 0
        let mut state = State::default();
        state.entries.insert(1, 0.0);
        state.entries.insert(2, 0.0);
        state.entries.insert(3, 0.0);

        // Apply partial evaluation
        let atol = ATol::new(1e-10).unwrap();
        let result = sos1.partial_evaluate(&state, atol).unwrap();

        // Check that we get an updated constraint with no variables (all removed)
        // This is valid for SOS1 (unlike OneHot)
        match result {
            Sos1PartialEvaluateResult::Updated(updated) => {
                assert_eq!(updated.variables.len(), 0); // All variables removed
            }
            _ => panic!("Expected Updated result when all variables are 0"),
        }
    }
}
