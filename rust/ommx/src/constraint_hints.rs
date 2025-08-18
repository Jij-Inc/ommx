mod one_hot;
mod sos1;

pub use one_hot::OneHot;
pub use sos1::Sos1;

use one_hot::OneHotPartialEvaluateResult;
use sos1::Sos1PartialEvaluateResult;

use crate::{
    parse::{Parse, ParseError},
    v1::{self, State},
    ATol, Constraint, ConstraintID, DecisionVariable, RemovedConstraint, VariableID,
};
use std::collections::BTreeMap;
use thiserror::Error;

/// Error that can occur when working with ConstraintHints
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum ConstraintHintsError {
    #[error("Multiple variables are fixed to non-zero values in OneHot constraint {constraint_id:?}: {variables:?}")]
    OneHotMultipleNonZeroFixed {
        constraint_id: ConstraintID,
        variables: Vec<(VariableID, f64)>,
    },
    #[error("Variable {variable_id:?} in OneHot constraint {constraint_id:?} is fixed to invalid value {value} (must be 0 or 1)")]
    OneHotInvalidFixedValue {
        constraint_id: ConstraintID,
        variable_id: VariableID,
        value: f64,
    },
    #[error("All variables in OneHot constraint {constraint_id:?} are fixed to 0, constraint cannot be satisfied")]
    OneHotAllVariablesFixedToZero { 
        constraint_id: ConstraintID 
    },
    #[error("Multiple variables are fixed to non-zero values in SOS1 constraint (binary: {binary_constraint_id:?}): {variables:?}")]
    Sos1MultipleNonZeroFixed {
        binary_constraint_id: ConstraintID,
        variables: Vec<(VariableID, f64)>,
    },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConstraintHints {
    pub one_hot_constraints: Vec<OneHot>,
    pub sos1_constraints: Vec<Sos1>,
}

impl ConstraintHints {
    pub fn is_empty(&self) -> bool {
        self.one_hot_constraints.is_empty() && self.sos1_constraints.is_empty()
    }

    /// Partially evaluate all constraint hints with the given state.
    ///
    /// This method modifies the constraint hints in-place by:
    /// - Removing constraints that are satisfied or cannot be satisfied
    /// - Updating constraints by removing variables fixed to 0
    ///
    /// Returns a new State containing the original state plus any additional
    /// variable fixings discovered through constraint propagation.
    ///
    /// The process iterates until no more variable fixings are discovered,
    /// ensuring all constraint propagations are applied.
    pub fn partial_evaluate(
        &mut self,
        mut state: State,
        atol: ATol,
    ) -> Result<State, ConstraintHintsError> {
        let mut changed = true;
        while changed {
            changed = false;
            let one_hot_constraints = std::mem::take(&mut self.one_hot_constraints);
            for one_hot in one_hot_constraints {
                match one_hot.partial_evaluate(&state, atol)? {
                    OneHotPartialEvaluateResult::Updated(updated) => {
                        self.one_hot_constraints.push(updated);
                    }
                    OneHotPartialEvaluateResult::AdditionalFix(additional_state) => {
                        for (var_id, value) in additional_state.entries {
                            state.entries.insert(var_id, value);
                        }
                        changed = true;
                    }
                }
            }

            let sos1_constraints = std::mem::take(&mut self.sos1_constraints);
            for sos1 in sos1_constraints {
                match sos1.partial_evaluate(&state, atol)? {
                    Sos1PartialEvaluateResult::Updated(updated) => {
                        self.sos1_constraints.push(updated);
                    }
                    Sos1PartialEvaluateResult::AdditionalFix(additional_state) => {
                        for (var_id, value) in additional_state.entries {
                            state.entries.insert(var_id, value);
                        }
                        changed = true;
                    }
                }
            }
        }

        Ok(state)
    }
}

impl Parse for v1::ConstraintHints {
    type Output = ConstraintHints;
    type Context = (
        BTreeMap<VariableID, DecisionVariable>,
        BTreeMap<ConstraintID, Constraint>,
        BTreeMap<ConstraintID, RemovedConstraint>,
    );
    fn parse(self, context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.ConstraintHints";
        let one_hot_constraints = self
            .one_hot_constraints
            .into_iter()
            .map(|c| c.parse_as(context, message, "one_hot_constraints"))
            .collect::<Result<Vec<_>, ParseError>>()?;
        let sos1_constraints = self
            .sos1_constraints
            .into_iter()
            .map(|c| c.parse_as(context, message, "sos1_constraints"))
            .collect::<Result<_, ParseError>>()?;
        Ok(ConstraintHints {
            one_hot_constraints,
            sos1_constraints,
        })
    }
}

impl From<ConstraintHints> for v1::ConstraintHints {
    fn from(value: ConstraintHints) -> Self {
        Self {
            one_hot_constraints: value
                .one_hot_constraints
                .into_iter()
                .map(|oh| oh.into())
                .collect(),
            sos1_constraints: value
                .sos1_constraints
                .into_iter()
                .map(|s| s.into())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_hints_partial_evaluate_propagation() {
        // Create constraint hints with OneHot and SOS1 constraints
        let mut hints = ConstraintHints {
            one_hot_constraints: vec![OneHot {
                id: ConstraintID::from(100),
                variables: vec![
                    VariableID::from(1),
                    VariableID::from(2),
                    VariableID::from(3),
                ]
                .into_iter()
                .collect(),
            }],
            sos1_constraints: vec![Sos1 {
                binary_constraint_id: ConstraintID::from(200),
                big_m_constraint_ids: Default::default(),
                variables: vec![
                    VariableID::from(4),
                    VariableID::from(5),
                    VariableID::from(6),
                ]
                .into_iter()
                .collect(),
            }],
        };

        // Create initial state where variable 2 is fixed to 1
        let mut initial_state = State::default();
        initial_state.entries.insert(2, 1.0);

        // Apply partial evaluation
        let final_state = hints.partial_evaluate(initial_state, ATol::default()).unwrap();

        // Check that variables 1 and 3 were fixed to 0 due to OneHot propagation
        assert_eq!(final_state.entries.get(&1), Some(&0.0));
        assert_eq!(final_state.entries.get(&2), Some(&1.0)); // Original
        assert_eq!(final_state.entries.get(&3), Some(&0.0));

        // Check that OneHot constraint was removed (satisfied)
        assert_eq!(hints.one_hot_constraints.len(), 0);

        // Check that SOS1 constraint remains unchanged
        assert_eq!(hints.sos1_constraints.len(), 1);
        assert_eq!(hints.sos1_constraints[0].variables.len(), 3);
    }

    #[test]
    fn test_constraint_hints_partial_evaluate_cascade() {
        // Create constraint hints where one constraint affects another
        let mut hints = ConstraintHints {
            one_hot_constraints: vec![OneHot {
                id: ConstraintID::from(100),
                variables: vec![VariableID::from(1), VariableID::from(2)]
                    .into_iter()
                    .collect(),
            }],
            sos1_constraints: vec![Sos1 {
                binary_constraint_id: ConstraintID::from(200),
                big_m_constraint_ids: Default::default(),
                variables: vec![
                    VariableID::from(2), // Same variable as in OneHot
                    VariableID::from(3),
                ]
                .into_iter()
                .collect(),
            }],
        };

        // Create initial state where variable 1 is fixed to 1
        let mut initial_state = State::default();
        initial_state.entries.insert(1, 1.0);

        // Apply partial evaluation
        let final_state = hints.partial_evaluate(initial_state, ATol::default()).unwrap();

        // Check propagation: 1=1 -> 2=0 (OneHot)
        assert_eq!(final_state.entries.get(&1), Some(&1.0)); // Original
        assert_eq!(final_state.entries.get(&2), Some(&0.0)); // Fixed by OneHot
                                                             // Variable 3 is not fixed because SOS1 allows all zeros

        // Check that OneHot constraint was removed (satisfied)
        assert_eq!(hints.one_hot_constraints.len(), 0);

        // Check that SOS1 constraint remains but with variable 2 removed
        assert_eq!(hints.sos1_constraints.len(), 1);
        assert_eq!(hints.sos1_constraints[0].variables.len(), 1); // Only variable 3 remains
        assert!(hints.sos1_constraints[0]
            .variables
            .contains(&VariableID::from(3)));
    }

    #[test]
    fn test_constraint_hints_partial_evaluate_error_propagation() {
        // Create constraint hints that will cause an error
        let mut hints = ConstraintHints {
            one_hot_constraints: vec![OneHot {
                id: ConstraintID::from(100),
                variables: vec![VariableID::from(1), VariableID::from(2)]
                    .into_iter()
                    .collect(),
            }],
            sos1_constraints: vec![],
        };

        // Create initial state where both variables are fixed to 1 (violates OneHot)
        let mut initial_state = State::default();
        initial_state.entries.insert(1, 1.0);
        initial_state.entries.insert(2, 1.0);

        // Apply partial evaluation
        let result = hints.partial_evaluate(initial_state, ATol::default());

        // Check that we get an error
        match result {
            Err(ConstraintHintsError::OneHotMultipleNonZeroFixed { .. }) => {}
            _ => panic!("Expected OneHot MultipleNonZeroFixed error"),
        }
    }

    #[test]
    fn test_constraint_hints_partial_evaluate_no_changes() {
        // Create constraint hints with no variables in state
        let mut hints = ConstraintHints {
            one_hot_constraints: vec![OneHot {
                id: ConstraintID::from(100),
                variables: vec![VariableID::from(1), VariableID::from(2)]
                    .into_iter()
                    .collect(),
            }],
            sos1_constraints: vec![],
        };

        // Create empty state
        let initial_state = State::default();

        // Apply partial evaluation
        let final_state = hints.partial_evaluate(initial_state, ATol::default()).unwrap();

        // Check that state remains empty
        assert_eq!(final_state.entries.len(), 0);

        // Check that constraints remain unchanged
        assert_eq!(hints.one_hot_constraints.len(), 1);
        assert_eq!(hints.one_hot_constraints[0].variables.len(), 2);
    }
}
