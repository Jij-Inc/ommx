use crate::{
    parse::{as_constraint_id, as_variable_id, Parse, ParseError, RawParseError},
    v1::{self, Samples, State},
    Constraint, ConstraintID, DecisionVariable, Evaluate, InstanceError, RemovedConstraint,
    VariableID, VariableIDSet,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sos1 {
    pub binary_constraint_id: ConstraintID,
    pub variables: BTreeSet<VariableID>,
    /// Map from variable ID to corresponding big-M constraint ID
    pub variable_to_big_m_constraint: BTreeMap<VariableID, ConstraintID>,
}

impl Sos1 {
    /// Get all big-M constraint IDs
    pub fn big_m_constraint_ids(&self) -> BTreeSet<ConstraintID> {
        self.variable_to_big_m_constraint
            .values()
            .cloned()
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
        let mut variables = BTreeSet::new();
        let mut parsed_var_ids = Vec::new();
        for id in &self.decision_variables {
            let id = as_variable_id(decision_variable, *id)
                .map_err(|e| e.context(message, "decision_variables"))?;
            if !variables.insert(id) {
                return Err(
                    RawParseError::InstanceError(InstanceError::NonUniqueVariableID { id })
                        .context(message, "decision_variables"),
                );
            }
            parsed_var_ids.push(id);
        }

        // Build variable to big-M constraint map
        // Assumes variables and big_m_constraint_ids have 1:1 correspondence
        let mut variable_to_big_m_constraint = BTreeMap::new();
        if parsed_var_ids.len() == parsed_big_m_ids.len() {
            for (var_id, constraint_id) in
                parsed_var_ids.into_iter().zip(parsed_big_m_ids.into_iter())
            {
                variable_to_big_m_constraint.insert(var_id, constraint_id);
            }
        }
        // If lengths don't match, leave the map empty (backward compatibility)

        Ok(Sos1 {
            binary_constraint_id,
            variables,
            variable_to_big_m_constraint,
        })
    }
}

impl From<Sos1> for v1::Sos1 {
    fn from(value: Sos1) -> Self {
        // Reconstruct the original ordering of big_m_constraint_ids and decision_variables
        let mut big_m_constraint_ids = Vec::new();
        let mut decision_variables = Vec::new();

        // We need to maintain the same order, so iterate through variables
        for var_id in &value.variables {
            decision_variables.push(**var_id);
            if let Some(constraint_id) = value.variable_to_big_m_constraint.get(var_id) {
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

impl Evaluate for Sos1 {
    type Output = ();
    type SampledOutput = ();

    fn evaluate(&self, _state: &State, _atol: crate::ATol) -> anyhow::Result<Self::Output> {
        Ok(())
    }

    fn evaluate_samples(
        &self,
        _samples: &Samples,
        _atol: crate::ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        Ok(())
    }

    fn partial_evaluate(&mut self, state: &State, atol: crate::ATol) -> anyhow::Result<()> {
        let mut variables_to_remove = Vec::new();
        let mut should_discard = false;

        for &var_id in &self.variables {
            if let Some(&value) = state.entries.get(&var_id.into_inner()) {
                if value.abs() < *atol {
                    // If the value is 0 (within tolerance), remove the variable
                    variables_to_remove.push(var_id);
                } else {
                    // If the value is non-zero, warn and discard the hint
                    log::warn!(
                        "Sos1 constraint hint with binary_constraint_id {} has variable {} with non-zero value {}. Discarding the hint.",
                        self.binary_constraint_id,
                        var_id,
                        value
                    );
                    should_discard = true;
                    break;
                }
            }
        }

        if should_discard {
            self.variables.clear();
            self.variable_to_big_m_constraint.clear();
        } else {
            for var in variables_to_remove {
                self.variables.remove(&var);
                // Remove corresponding big-M constraint from the map
                self.variable_to_big_m_constraint.remove(&var);
            }
        }

        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        self.variables.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::State;
    use maplit::{btreemap, btreeset, hashmap};

    #[test]
    fn test_sos1_partial_evaluate_remove_zero() {
        // Test that Sos1 removes variables with value 0 and their corresponding big-M constraints
        let mut sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(1),
            variables: btreeset! {
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            },
            variable_to_big_m_constraint: btreemap! {
                VariableID::from(1) => ConstraintID::from(10),
                VariableID::from(2) => ConstraintID::from(20),
                VariableID::from(3) => ConstraintID::from(30),
            },
        };

        let state = State {
            entries: hashmap! {
                1 => 0.0,  // Should be removed with constraint 10
                2 => 0.0,  // Should be removed with constraint 20
            },
        };

        sos1.partial_evaluate(&state, crate::ATol::default())
            .unwrap();

        // Only variable 3 should remain
        assert_eq!(sos1.variables.len(), 1);
        assert!(sos1.variables.contains(&VariableID::from(3)));

        // Only constraint 30 should remain in the map
        assert_eq!(sos1.variable_to_big_m_constraint.len(), 1);
        assert_eq!(
            sos1.variable_to_big_m_constraint.get(&VariableID::from(3)),
            Some(&ConstraintID::from(30))
        );

        // Check big_m_constraint_ids() method
        let big_m_ids = sos1.big_m_constraint_ids();
        assert_eq!(big_m_ids.len(), 1);
        assert!(big_m_ids.contains(&ConstraintID::from(30)));
    }

    #[test]
    fn test_sos1_partial_evaluate_discard_nonzero() {
        // Test that Sos1 is discarded when a variable has non-zero value
        let mut sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(1),
            variables: btreeset! {
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            },
            variable_to_big_m_constraint: btreemap! {
                VariableID::from(1) => ConstraintID::from(10),
                VariableID::from(2) => ConstraintID::from(20),
                VariableID::from(3) => ConstraintID::from(30),
            },
        };

        let state = State {
            entries: hashmap! {
                1 => 0.0,
                2 => 0.5,  // Non-zero value should cause discard
            },
        };

        sos1.partial_evaluate(&state, crate::ATol::default())
            .unwrap();

        // All fields should be cleared
        assert!(sos1.variables.is_empty());
        assert!(sos1.variable_to_big_m_constraint.is_empty());
        assert!(sos1.big_m_constraint_ids().is_empty());
    }
}
