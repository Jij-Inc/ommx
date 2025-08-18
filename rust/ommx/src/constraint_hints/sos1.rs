use crate::{
    parse::{as_constraint_id, as_variable_id, Parse, ParseError, RawParseError},
    v1::{self, State},
    ATol, Constraint, ConstraintID, DecisionVariable, InstanceError, RemovedConstraint, VariableID,
};
use std::collections::{BTreeMap, BTreeSet};

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
                        "SOS1 constraint (binary: {:?}): variable {:?} is fixed to non-zero value {}, discarding the entire hint",
                        self.binary_constraint_id,
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
        assert_eq!(result.variables.len(), 2);
        assert!(result.variables.contains(&VariableID::from(1)));
        assert!(!result.variables.contains(&VariableID::from(2)));
        assert!(result.variables.contains(&VariableID::from(3)));
        // Check that constraint IDs are unchanged
        assert_eq!(result.binary_constraint_id, ConstraintID::from(100));
        assert_eq!(result.big_m_constraint_ids.len(), 3);
    }

    #[test]
    fn test_partial_evaluate_discards_on_nonzero_fixed() {
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
        let result = sos1.partial_evaluate(&state, atol);

        // Check that the constraint was discarded
        assert!(result.is_none());
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
        assert_eq!(result.variables.len(), 2);
        assert!(!result.variables.contains(&VariableID::from(2)));
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
        assert_eq!(result.variables, expected_variables);
        assert_eq!(result.binary_constraint_id, expected_binary_id);
        assert_eq!(result.big_m_constraint_ids, expected_big_m_ids);
    }
}
