use crate::{
    parse::{as_constraint_id, as_variable_id, Parse, ParseError, RawParseError},
    v1::{self, Samples, State},
    Constraint, ConstraintID, DecisionVariable, Evaluate, InstanceError, RemovedConstraint,
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

impl Evaluate for OneHot {
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
                        "OneHot constraint hint {} has variable {} with non-zero value {}. Discarding the hint.",
                        self.id,
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
        } else {
            for var in variables_to_remove {
                self.variables.remove(&var);
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
    use maplit::{btreeset, hashmap};

    #[test]
    fn test_one_hot_partial_evaluate_remove_zero() {
        // Test that OneHot removes variables with value 0
        let mut one_hot = OneHot {
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

        one_hot
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();

        // Only variable 3 should remain
        assert_eq!(one_hot.variables.len(), 1);
        assert!(one_hot.variables.contains(&VariableID::from(3)));
    }

    #[test]
    fn test_one_hot_partial_evaluate_discard_nonzero() {
        // Test that OneHot is discarded when a variable has non-zero value
        let mut one_hot = OneHot {
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

        one_hot
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();

        // All variables should be cleared
        assert!(one_hot.variables.is_empty());
    }
}
