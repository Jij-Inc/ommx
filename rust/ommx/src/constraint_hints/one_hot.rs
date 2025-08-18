use crate::{
    parse::{as_constraint_id, as_variable_id, Parse, ParseError, RawParseError},
    v1::{self, State},
    ATol, Constraint, ConstraintID, DecisionVariable, InstanceError, RemovedConstraint, VariableID,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Result of partial evaluation for OneHot constraint
#[derive(Debug, Clone, PartialEq)]
pub enum OneHotPartialEvaluateResult {
    /// Constraint was updated by removing zero variables
    Updated(OneHot),
    /// A variable was fixed to 1, so the constraint is satisfied
    /// Returns a State with variables to be fixed to 0
    AdditionalFix(State),
}

/// Error that can occur during partial evaluation of OneHot constraint
#[derive(Debug, Clone, Error)]
pub enum OneHotPartialEvaluateError {
    #[error("Multiple variables are fixed to non-zero values in OneHot constraint {constraint_id:?}: {variables:?}")]
    MultipleNonZeroFixed {
        constraint_id: ConstraintID,
        variables: Vec<(VariableID, f64)>,
    },
    #[error("Variable {variable_id:?} in OneHot constraint {constraint_id:?} is fixed to invalid value {value} (must be 0 or 1)")]
    InvalidFixedValue {
        constraint_id: ConstraintID,
        variable_id: VariableID,
        value: f64,
    },
    #[error("All variables in OneHot constraint {constraint_id:?} are fixed to 0, constraint cannot be satisfied")]
    AllVariablesFixedToZero { constraint_id: ConstraintID },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneHot {
    pub id: ConstraintID,
    pub variables: BTreeSet<VariableID>,
}

impl OneHot {
    /// Partially evaluate the OneHot constraint with given state.
    ///
    /// - If a decision variable is assigned 0, it is removed from the constraint
    /// - If exactly one variable is fixed to 1, other variables are fixed to 0
    /// - If a variable is fixed to a value other than 0 or 1, returns an error
    /// - If multiple variables are fixed to non-zero values, returns an error
    ///
    /// Returns a result indicating whether the constraint was updated, requires additional fixes, or should be removed.
    pub fn partial_evaluate(
        mut self,
        state: &State,
        atol: ATol,
    ) -> Result<OneHotPartialEvaluateResult, OneHotPartialEvaluateError> {
        let mut fixed_to_one: Option<VariableID> = None;
        let mut variables_to_remove = Vec::new();

        // Check each variable in the OneHot constraint
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

            // Variable is approximately one
            if (value - 1.0).abs() < atol {
                // Check if another variable was already fixed to one
                if let Some(first_var) = fixed_to_one {
                    return Err(OneHotPartialEvaluateError::MultipleNonZeroFixed {
                        constraint_id: self.id,
                        variables: vec![(first_var, 1.0), (var_id, value)],
                    });
                }
                fixed_to_one = Some(var_id);
                variables_to_remove.push(var_id);
                continue;
            }

            // Variable is fixed to an invalid value (not 0 or 1)
            return Err(OneHotPartialEvaluateError::InvalidFixedValue {
                constraint_id: self.id,
                variable_id: var_id,
                value,
            });
        }

        // Remove variables that are fixed
        for var_id in variables_to_remove {
            self.variables.remove(&var_id);
        }

        // Handle the different cases
        if fixed_to_one.is_some() {
            // One variable is fixed to 1, need to fix remaining variables to 0
            let mut additional_fixes = State::default();
            for &var_id in &self.variables {
                additional_fixes.entries.insert(*var_id, 0.0);
            }
            Ok(OneHotPartialEvaluateResult::AdditionalFix(additional_fixes))
        } else if self.variables.is_empty() {
            // All variables were fixed to 0, constraint cannot be satisfied
            Err(OneHotPartialEvaluateError::AllVariablesFixedToZero {
                constraint_id: self.id,
            })
        } else {
            // Some variables remain unfixed
            Ok(OneHotPartialEvaluateResult::Updated(self))
        }
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
        match result {
            OneHotPartialEvaluateResult::Updated(updated) => {
                assert_eq!(updated.variables.len(), 2);
                assert!(updated.variables.contains(&VariableID::from(1)));
                assert!(!updated.variables.contains(&VariableID::from(2)));
                assert!(updated.variables.contains(&VariableID::from(3)));
            }
            _ => panic!("Expected Updated result"),
        }
    }

    #[test]
    fn test_partial_evaluate_fixes_others_when_one_is_fixed() {
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

        // Create a state where variable 2 is set to 1.0
        let mut state = State::default();
        state.entries.insert(2, 1.0);

        // Apply partial evaluation
        let atol = ATol::new(1e-10).unwrap();
        let result = one_hot.partial_evaluate(&state, atol).unwrap();

        // Check that we get additional fixes for other variables
        match result {
            OneHotPartialEvaluateResult::AdditionalFix(fixes) => {
                assert_eq!(fixes.entries.len(), 2); // Two variables to fix
                assert_eq!(fixes.entries.get(&1), Some(&0.0));
                assert_eq!(fixes.entries.get(&3), Some(&0.0));
            }
            _ => panic!("Expected AdditionalFix result"),
        }
    }

    #[test]
    fn test_partial_evaluate_error_on_invalid_value() {
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

        // Create a state where variable 2 is set to 0.5 (invalid)
        let mut state = State::default();
        state.entries.insert(2, 0.5);

        // Apply partial evaluation
        let atol = ATol::new(1e-10).unwrap();
        let result = one_hot.partial_evaluate(&state, atol);

        // Check that we get an error
        match result {
            Err(OneHotPartialEvaluateError::InvalidFixedValue {
                variable_id, value, ..
            }) => {
                assert_eq!(variable_id, VariableID::from(2));
                assert_eq!(value, 0.5);
            }
            _ => panic!("Expected InvalidFixedValue error"),
        }
    }

    #[test]
    fn test_partial_evaluate_error_on_multiple_ones() {
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

        // Create a state where variables 1 and 2 are both set to 1.0
        let mut state = State::default();
        state.entries.insert(1, 1.0);
        state.entries.insert(2, 1.0);

        // Apply partial evaluation
        let atol = ATol::new(1e-10).unwrap();
        let result = one_hot.partial_evaluate(&state, atol);

        // Check that we get an error
        match result {
            Err(OneHotPartialEvaluateError::MultipleNonZeroFixed { variables, .. }) => {
                assert_eq!(variables.len(), 2);
            }
            _ => panic!("Expected MultipleNonZeroFixed error"),
        }
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
        match result {
            OneHotPartialEvaluateResult::Updated(updated) => {
                assert_eq!(updated.variables.len(), 2);
                assert!(!updated.variables.contains(&VariableID::from(2)));
            }
            _ => panic!("Expected Updated result"),
        }
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
        match result {
            OneHotPartialEvaluateResult::Updated(updated) => {
                assert_eq!(updated.variables.len(), 3);
                assert_eq!(updated.variables, expected_variables);
            }
            _ => panic!("Expected Updated result"),
        }
    }

    #[test]
    fn test_partial_evaluate_all_zeros_error() {
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

        // Create a state where all variables are set to 0
        let mut state = State::default();
        state.entries.insert(1, 0.0);
        state.entries.insert(2, 0.0);
        state.entries.insert(3, 0.0);

        // Apply partial evaluation
        let atol = ATol::new(1e-10).unwrap();
        let result = one_hot.partial_evaluate(&state, atol);

        // Check that we get an error
        match result {
            Err(OneHotPartialEvaluateError::AllVariablesFixedToZero { constraint_id }) => {
                assert_eq!(constraint_id, ConstraintID::from(100));
            }
            _ => panic!("Expected AllVariablesFixedToZero error when all variables are 0"),
        }
    }
}
