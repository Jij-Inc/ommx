use super::*;
use crate::{ATol, Evaluate, Propagate, PropagateOutcome, VariableIDSet};

impl Propagate for OneHotConstraint<Created> {
    type Transformed = std::convert::Infallible;

    fn propagate(
        mut self,
        state: &crate::v1::State,
        atol: ATol,
    ) -> crate::Result<(PropagateOutcome<Self>, crate::v1::State)> {
        let mut fixed_to_one: Option<VariableID> = None;
        let mut unfixed = BTreeSet::new();

        for &var_id in &self.variables {
            let Some(&value) = state.entries.get(&var_id.into_inner()) else {
                unfixed.insert(var_id);
                continue;
            };

            if (value - 1.0).abs() < *atol {
                // Variable is ~1
                if let Some(first) = fixed_to_one {
                    crate::bail!(
                        "Multiple variables fixed to 1 in one-hot constraint: {:?} and {:?}",
                        first,
                        var_id
                    );
                }
                fixed_to_one = Some(var_id);
            } else if value.abs() < *atol {
                // Variable is ~0, removed from set
            } else {
                crate::bail!(
                    "Variable {:?} in one-hot constraint fixed to invalid value {} (must be 0 or 1)",
                    var_id,
                    value
                );
            }
        }

        if fixed_to_one.is_some() {
            // One variable is 1 → constraint satisfied, fix remaining unfixed to 0
            let mut additional = crate::v1::State::default();
            for var_id in &unfixed {
                additional.entries.insert(var_id.into_inner(), 0.0);
            }
            Ok((PropagateOutcome::Consumed(self), additional))
        } else if unfixed.is_empty() {
            // All variables fixed to 0 → infeasible
            crate::bail!(
                "All variables in one-hot constraint are fixed to 0, constraint cannot be satisfied"
            );
        } else if unfixed.len() == 1 {
            // Unit propagation: exactly one unfixed variable → must be 1
            let var_id = *unfixed.iter().next().unwrap();
            let mut additional = crate::v1::State::default();
            additional.entries.insert(var_id.into_inner(), 1.0);
            Ok((PropagateOutcome::Consumed(self), additional))
        } else {
            // Multiple unfixed variables remain — modify and stay active
            self.variables = unfixed;
            Ok((PropagateOutcome::Active(self), crate::v1::State::default()))
        }
    }
}

impl Evaluate for OneHotConstraint<Created> {
    type Output = EvaluatedOneHotConstraint;
    type SampledOutput = SampledOneHotConstraint;

    fn evaluate(&self, state: &crate::v1::State, atol: ATol) -> crate::Result<Self::Output> {
        let used_decision_variable_ids = self.required_ids();
        let (feasible, active_variable) = check_one_hot(&self.variables, state, atol)?;

        Ok(OneHotConstraint {
            variables: self.variables.clone(),
            metadata: self.metadata.clone(),
            stage: OneHotEvaluatedData {
                feasible,
                active_variable,
                used_decision_variable_ids,
            },
        })
    }

    fn evaluate_samples(
        &self,
        samples: &crate::Sampled<crate::v1::State>,
        atol: ATol,
    ) -> crate::Result<Self::SampledOutput> {
        let mut feasible = BTreeMap::new();
        let mut active_variable = BTreeMap::new();

        for (sample_id, state) in samples.iter() {
            let (f, av) = check_one_hot(&self.variables, state, atol)?;
            feasible.insert(*sample_id, f);
            active_variable.insert(*sample_id, av);
        }

        Ok(OneHotConstraint {
            variables: self.variables.clone(),
            metadata: self.metadata.clone(),
            stage: OneHotSampledData {
                feasible,
                active_variable,
                used_decision_variable_ids: self.required_ids(),
            },
        })
    }

    fn partial_evaluate(&mut self, state: &crate::v1::State, _atol: ATol) -> crate::Result<()> {
        for var_id in &self.variables {
            if state.entries.contains_key(&var_id.into_inner()) {
                crate::bail!(
                    "Cannot partially evaluate variable {:?} of one-hot constraint. \
                     Fixing a one-hot variable would change the constraint type.",
                    var_id
                );
            }
        }
        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        self.variables.iter().copied().collect()
    }
}

/// Check one-hot feasibility for a single state.
///
/// Returns `(feasible, active_variable)`:
/// - feasible: exactly one variable is 1, the rest are 0
/// - active_variable: the variable that is 1 (None if infeasible)
fn check_one_hot(
    variables: &BTreeSet<VariableID>,
    state: &crate::v1::State,
    atol: ATol,
) -> crate::Result<(bool, Option<VariableID>)> {
    let mut active: Option<VariableID> = None;

    for &var_id in variables {
        let value = state.entries.get(&var_id.into_inner()).ok_or_else(|| {
            crate::error!(
                "Variable {:?} not found in state for one-hot constraint",
                var_id,
            )
        })?;

        if (value - 1.0).abs() < *atol {
            // Variable is ~1
            if active.is_some() {
                // Multiple variables are 1 → infeasible
                return Ok((false, None));
            }
            active = Some(var_id);
        } else if value.abs() < *atol {
            // Variable is ~0, OK
        } else {
            // Variable is neither 0 nor 1 → infeasible
            return Ok((false, None));
        }
    }

    match active {
        Some(var_id) => Ok((true, Some(var_id))),
        None => Ok((false, None)), // All zeros → infeasible for one-hot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Evaluate, Propagate, PropagateOutcome};
    use std::collections::HashMap;

    fn make_one_hot(_id: u64, var_ids: &[u64]) -> OneHotConstraint {
        let vars = var_ids.iter().copied().map(VariableID::from).collect();
        OneHotConstraint::new(vars)
    }

    #[test]
    fn test_evaluate_feasible() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x1=0, x2=1, x3=0 → feasible, active=x2
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 1.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(result.stage.feasible);
        assert_eq!(result.stage.active_variable, Some(VariableID::from(2)));
    }

    #[test]
    fn test_evaluate_infeasible_multiple_ones() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x1=1, x2=1, x3=0 → infeasible
        let state = crate::v1::State::from(HashMap::from([(1, 1.0), (2, 1.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(!result.stage.feasible);
        assert_eq!(result.stage.active_variable, None);
    }

    #[test]
    fn test_evaluate_infeasible_all_zeros() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x1=0, x2=0, x3=0 → infeasible (one-hot requires exactly one)
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 0.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(!result.stage.feasible);
        assert_eq!(result.stage.active_variable, None);
    }

    #[test]
    fn test_evaluate_infeasible_non_binary() {
        let c = make_one_hot(1, &[1, 2]);
        // x1=0.5, x2=0.5 → infeasible
        let state = crate::v1::State::from(HashMap::from([(1, 0.5), (2, 0.5)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(!result.stage.feasible);
    }

    #[test]
    fn test_partial_evaluate_error() {
        let mut c = make_one_hot(1, &[1, 2, 3]);
        let state = crate::v1::State::from(HashMap::from([(2, 1.0)]));
        let result = c.partial_evaluate(&state, ATol::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_partial_evaluate_no_overlap() {
        let mut c = make_one_hot(1, &[1, 2, 3]);
        // Fixing a variable NOT in the one-hot set is fine
        let state = crate::v1::State::from(HashMap::from([(99, 1.0)]));
        let result = c.partial_evaluate(&state, ATol::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_required_ids() {
        let c = make_one_hot(1, &[1, 2, 3]);
        let ids = c.required_ids();
        assert!(ids.contains(&VariableID::from(1)));
        assert!(ids.contains(&VariableID::from(2)));
        assert!(ids.contains(&VariableID::from(3)));
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_evaluate_samples() {
        let c = make_one_hot(1, &[1, 2, 3]);

        let mut samples = crate::Sampled::<crate::v1::State>::default();
        // Sample 0: x1=1, x2=0, x3=0 → feasible, active=x1
        samples
            .append(
                [crate::SampleID::from(0)],
                crate::v1::State::from(HashMap::from([(1, 1.0), (2, 0.0), (3, 0.0)])),
            )
            .unwrap();
        // Sample 1: x1=1, x2=1, x3=0 → infeasible
        samples
            .append(
                [crate::SampleID::from(1)],
                crate::v1::State::from(HashMap::from([(1, 1.0), (2, 1.0), (3, 0.0)])),
            )
            .unwrap();
        // Sample 2: x1=0, x2=0, x3=0 → infeasible (all zeros)
        samples
            .append(
                [crate::SampleID::from(2)],
                crate::v1::State::from(HashMap::from([(1, 0.0), (2, 0.0), (3, 0.0)])),
            )
            .unwrap();

        let result = c.evaluate_samples(&samples, ATol::default()).unwrap();

        let s0 = crate::SampleID::from(0);
        let s1 = crate::SampleID::from(1);
        let s2 = crate::SampleID::from(2);

        assert!(result.stage.feasible[&s0]);
        assert!(!result.stage.feasible[&s1]);
        assert!(!result.stage.feasible[&s2]);

        assert_eq!(result.stage.active_variable[&s0], Some(VariableID::from(1)));
        assert_eq!(result.stage.active_variable[&s1], None);
        assert_eq!(result.stage.active_variable[&s2], None);
    }

    // === Propagate tests ===

    #[test]
    fn test_propagate_var_one_fixes_rest() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x2=1 → Consumed, fix x1=0, x3=0
        let state = crate::v1::State::from(HashMap::from([(2, 1.0)]));
        let (outcome, additional) = c.propagate(&state, ATol::default()).unwrap();
        match outcome {
            PropagateOutcome::Consumed(original) => {
                assert_eq!(original.variables.len(), 3);
            }
            _ => panic!("Expected Consumed"),
        }
        assert_eq!(additional.entries.get(&1), Some(&0.0));
        assert_eq!(additional.entries.get(&3), Some(&0.0));
        assert_eq!(additional.entries.len(), 2);
    }

    #[test]
    fn test_propagate_var_zero_shrinks() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x1=0 → Active (shrunk to {x2, x3})
        let state = crate::v1::State::from(HashMap::from([(1, 0.0)]));
        let (outcome, additional) = c.propagate(&state, ATol::default()).unwrap();
        match outcome {
            PropagateOutcome::Active(c) => {
                assert_eq!(c.variables.len(), 2);
                assert!(c.variables.contains(&VariableID::from(2)));
                assert!(c.variables.contains(&VariableID::from(3)));
            }
            _ => panic!("Expected Active"),
        }
        assert!(additional.entries.is_empty());
    }

    #[test]
    fn test_propagate_unit_clause() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // x1=0, x2=0 → only x3 unfixed → unit propagation: x3=1, constraint consumed
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 0.0)]));
        let (outcome, additional) = c.propagate(&state, ATol::default()).unwrap();
        assert!(matches!(outcome, PropagateOutcome::Consumed(_)));
        assert_eq!(additional.entries.get(&3), Some(&1.0));
        assert_eq!(additional.entries.len(), 1);
    }

    #[test]
    fn test_propagate_all_zeros_error() {
        let c = make_one_hot(1, &[1, 2, 3]);
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 0.0), (3, 0.0)]));
        let result = c.propagate(&state, ATol::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_propagate_multiple_ones_error() {
        let c = make_one_hot(1, &[1, 2, 3]);
        let state = crate::v1::State::from(HashMap::from([(1, 1.0), (2, 1.0)]));
        let result = c.propagate(&state, ATol::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_propagate_no_overlap() {
        let c = make_one_hot(1, &[1, 2, 3]);
        // No variables in state overlap → Active (unchanged)
        let state = crate::v1::State::from(HashMap::from([(99, 5.0)]));
        let (outcome, additional) = c.propagate(&state, ATol::default()).unwrap();
        match outcome {
            PropagateOutcome::Active(c) => {
                assert_eq!(c.variables.len(), 3);
            }
            _ => panic!("Expected Active"),
        }
        assert!(additional.entries.is_empty());
    }
}
