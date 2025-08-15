use crate::{
    parse::{as_constraint_id, as_variable_id, Parse, ParseError, RawParseError},
    v1::{self, State},
    Constraint, ConstraintID, ConstraintIDSet, DecisionVariable, InstanceError, RemovedConstraint,
    VariableID, VariableIDSet,
};
use getset::Getters;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Getters)]
pub struct Sos1 {
    #[getset(get = "pub")]
    binary_constraint_id: ConstraintID,
    /// Map from variable ID to corresponding big-M constraint ID (None if no big-M constraint)
    #[getset(get = "pub")]
    variable_to_big_m_constraint: BTreeMap<VariableID, Option<ConstraintID>>,
}

impl Sos1 {
    /// Create new Sos1 constraint hint
    pub fn new(
        binary_constraint_id: ConstraintID,
        variable_to_big_m_constraint: BTreeMap<VariableID, Option<ConstraintID>>,
    ) -> Self {
        Self {
            binary_constraint_id,
            variable_to_big_m_constraint,
        }
    }

    /// Get all variable IDs in this SOS1 constraint
    pub fn variables(&self) -> BTreeSet<VariableID> {
        self.variable_to_big_m_constraint.keys().cloned().collect()
    }

    /// Get all big-M constraint IDs (excluding None values)
    pub fn big_m_constraint_ids(&self) -> BTreeSet<ConstraintID> {
        self.variable_to_big_m_constraint
            .values()
            .filter_map(|opt| *opt)
            .collect()
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

        // Parse big_m_constraint_ids
        let mut parsed_big_m_ids = Vec::new();
        for id in &self.big_m_constraint_ids {
            let id = as_constraint_id(constraints, removed_constraints, *id)
                .map_err(|e| e.context(message, "big_m_constraint_ids"))?;
            parsed_big_m_ids.push(id);
        }

        // Parse variables
        let mut parsed_var_ids = Vec::new();
        for id in &self.decision_variables {
            let id = as_variable_id(decision_variable, *id)
                .map_err(|e| e.context(message, "decision_variables"))?;
            parsed_var_ids.push(id);
        }

        // Check for unique variable IDs
        let unique_vars: BTreeSet<_> = parsed_var_ids.iter().cloned().collect();
        if unique_vars.len() != parsed_var_ids.len() {
            return Err(
                RawParseError::InstanceError(InstanceError::NonUniqueVariableID {
                    id: *parsed_var_ids
                        .iter()
                        .find(|&id| parsed_var_ids.iter().filter(|&&v| v == *id).count() > 1)
                        .unwrap(),
                })
                .context(message, "decision_variables"),
            );
        }

        // Build variable to big-M constraint map
        let mut variable_to_big_m_constraint = BTreeMap::new();
        for (i, var_id) in parsed_var_ids.into_iter().enumerate() {
            let big_m_constraint = if i < parsed_big_m_ids.len() {
                Some(parsed_big_m_ids[i])
            } else {
                None
            };
            variable_to_big_m_constraint.insert(var_id, big_m_constraint);
        }

        Ok(Sos1::new(
            binary_constraint_id,
            variable_to_big_m_constraint,
        ))
    }
}

impl From<Sos1> for v1::Sos1 {
    fn from(value: Sos1) -> Self {
        // Reconstruct the original ordering of big_m_constraint_ids and decision_variables
        let mut big_m_constraint_ids = Vec::new();
        let mut decision_variables = Vec::new();

        // We need to maintain the same order, so iterate through the map
        for (var_id, big_m_constraint) in &value.variable_to_big_m_constraint {
            decision_variables.push(**var_id);
            if let Some(constraint_id) = big_m_constraint {
                big_m_constraint_ids.push(**constraint_id);
            }
        }

        Self {
            binary_constraint_id: *value.binary_constraint_id,
            big_m_constraint_ids,
            decision_variables,
        }
    }
}

impl Sos1 {
    /// Apply partial evaluation to this Sos1 constraint hint.
    /// Returns Some(updated_hint) if the hint should be kept, None if it should be discarded.
    pub fn partial_evaluate(mut self, state: &State, atol: crate::ATol) -> Option<Self> {
        let mut variables_to_remove = Vec::new();

        for var_id in self.variable_to_big_m_constraint.keys() {
            if let Some(&value) = state.entries.get(&var_id.into_inner()) {
                if value.abs() < *atol {
                    // If the value is 0 (within tolerance), remove the variable
                    variables_to_remove.push(*var_id);
                } else {
                    // If the value is non-zero, warn and discard the hint
                    log::warn!(
                        "Sos1 constraint hint with binary_constraint_id {} has variable {} with non-zero value {}. Discarding the hint.",
                        self.binary_constraint_id,
                        var_id,
                        value
                    );
                    return None; // Discard the hint
                }
            }
        }

        // Remove variables with zero values
        for var in variables_to_remove {
            self.variable_to_big_m_constraint.remove(&var);
        }

        Some(self) // Keep the updated hint
    }

    /// Get all decision variable IDs used by this constraint hint
    pub fn used_decision_variable_ids(&self) -> VariableIDSet {
        self.variables()
    }

    /// Get all constraint IDs used by this constraint hint
    pub fn used_constraint_ids(&self) -> ConstraintIDSet {
        let mut constraint_ids = ConstraintIDSet::new();
        constraint_ids.insert(self.binary_constraint_id);
        constraint_ids.extend(self.big_m_constraint_ids());
        constraint_ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::State;
    use maplit::{btreemap, hashmap};

    #[test]
    fn test_sos1_partial_evaluate_remove_zero() {
        // Test that Sos1 removes variables with value 0 and their corresponding big-M constraints
        let sos1 = Sos1::new(
            ConstraintID::from(1),
            btreemap! {
                VariableID::from(1) => Some(ConstraintID::from(10)),
                VariableID::from(2) => Some(ConstraintID::from(20)),
                VariableID::from(3) => Some(ConstraintID::from(30)),
            },
        );

        let state = State {
            entries: hashmap! {
                1 => 0.0,  // Should be removed with constraint 10
                2 => 0.0,  // Should be removed with constraint 20
            },
        };

        let result = sos1.partial_evaluate(&state, crate::ATol::default());

        // Should keep the hint and only variable 3 should remain
        assert!(result.is_some());
        let updated_hint = result.unwrap();
        assert_eq!(updated_hint.variables().len(), 1);
        assert!(updated_hint.variables().contains(&VariableID::from(3)));

        // Only constraint 30 should remain in the map
        assert_eq!(updated_hint.variable_to_big_m_constraint().len(), 1);
        assert_eq!(
            updated_hint
                .variable_to_big_m_constraint()
                .get(&VariableID::from(3)),
            Some(&Some(ConstraintID::from(30)))
        );

        // Check big_m_constraint_ids() method
        let big_m_ids = updated_hint.big_m_constraint_ids();
        assert_eq!(big_m_ids.len(), 1);
        assert!(big_m_ids.contains(&ConstraintID::from(30)));
    }

    #[test]
    fn test_sos1_partial_evaluate_discard_nonzero() {
        // Test that Sos1 is discarded when a variable has non-zero value
        let sos1 = Sos1::new(
            ConstraintID::from(1),
            btreemap! {
                VariableID::from(1) => Some(ConstraintID::from(10)),
                VariableID::from(2) => Some(ConstraintID::from(20)),
                VariableID::from(3) => Some(ConstraintID::from(30)),
            },
        );

        let state = State {
            entries: hashmap! {
                1 => 0.0,
                2 => 0.5,  // Non-zero value should cause discard
            },
        };

        let result = sos1.partial_evaluate(&state, crate::ATol::default());

        // Should discard the hint
        assert!(result.is_none());
    }

    #[test]
    fn test_sos1_with_none_big_m_constraints() {
        // Test that Sos1 can have variables without big-M constraints (None values)
        let sos1 = Sos1::new(
            ConstraintID::from(1),
            btreemap! {
                VariableID::from(1) => Some(ConstraintID::from(10)),
                VariableID::from(2) => None,  // No big-M constraint
                VariableID::from(3) => Some(ConstraintID::from(30)),
            },
        );

        // Should have 3 variables
        assert_eq!(sos1.variables().len(), 3);
        assert!(sos1.variables().contains(&VariableID::from(1)));
        assert!(sos1.variables().contains(&VariableID::from(2)));
        assert!(sos1.variables().contains(&VariableID::from(3)));

        // Should have only 2 big-M constraints (excluding None)
        let big_m_ids = sos1.big_m_constraint_ids();
        assert_eq!(big_m_ids.len(), 2);
        assert!(big_m_ids.contains(&ConstraintID::from(10)));
        assert!(big_m_ids.contains(&ConstraintID::from(30)));
    }

    #[test]
    fn test_sos1_all_none_big_m_constraints() {
        // Test that Sos1 works with all None big-M constraints
        let sos1 = Sos1::new(
            ConstraintID::from(1),
            btreemap! {
                VariableID::from(1) => None,
                VariableID::from(2) => None,
                VariableID::from(3) => None,
            },
        );

        // Should have 3 variables
        assert_eq!(sos1.variables().len(), 3);

        // Should have no big-M constraints
        let big_m_ids = sos1.big_m_constraint_ids();
        assert_eq!(big_m_ids.len(), 0);

        // Test used_constraint_ids should only include binary constraint
        let used_constraint_ids = sos1.used_constraint_ids();
        assert_eq!(used_constraint_ids.len(), 1);
        assert!(used_constraint_ids.contains(&ConstraintID::from(1)));
    }
}
