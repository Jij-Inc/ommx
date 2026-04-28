use super::*;
use crate::{ATol, Evaluate, Propagate, PropagateOutcome, VariableIDSet};

impl Propagate for Sos1Constraint<Created> {
    type Transformed = std::convert::Infallible;

    fn propagate(
        mut self,
        state: &crate::v1::State,
        atol: ATol,
    ) -> crate::Result<(PropagateOutcome<Self>, crate::v1::State)> {
        let mut fixed_nonzero: Option<VariableID> = None;
        let mut unfixed = BTreeSet::new();

        for &var_id in &self.variables {
            let Some(&value) = state.entries.get(&var_id.into_inner()) else {
                unfixed.insert(var_id);
                continue;
            };

            if value.abs() < *atol {
                // Variable is ~0, removed from set
            } else {
                // Variable is non-zero
                if let Some(first) = fixed_nonzero {
                    crate::bail!(
                        "Multiple variables fixed to non-zero in SOS1 constraint: {:?} and {:?}",
                        first,
                        var_id
                    );
                }
                fixed_nonzero = Some(var_id);
            }
        }

        if fixed_nonzero.is_some() {
            // One variable is non-zero → constraint satisfied, fix remaining unfixed to 0
            let mut additional = crate::v1::State::default();
            for var_id in &unfixed {
                additional.entries.insert(var_id.into_inner(), 0.0);
            }
            Ok((PropagateOutcome::Consumed(self), additional))
        } else if unfixed.is_empty() {
            // All variables fixed to 0 → vacuously satisfied for SOS1
            Ok((
                PropagateOutcome::Consumed(self),
                crate::v1::State::default(),
            ))
        } else {
            // Multiple unfixed variables remain — modify and stay active
            self.variables = unfixed;
            Ok((PropagateOutcome::Active(self), crate::v1::State::default()))
        }
    }
}

impl Evaluate for Sos1Constraint<Created> {
    type Output = EvaluatedSos1Constraint;
    type SampledOutput = SampledSos1Constraint;

    fn evaluate(&self, state: &crate::v1::State, atol: ATol) -> crate::Result<Self::Output> {
        let used_decision_variable_ids = self.required_ids();
        let (feasible, active_variable) = check_sos1(&self.variables, state, atol)?;

        Ok(Sos1Constraint {
            variables: self.variables.clone(),
            stage: Sos1EvaluatedData {
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
            let (f, av) = check_sos1(&self.variables, state, atol)?;
            feasible.insert(*sample_id, f);
            active_variable.insert(*sample_id, av);
        }

        Ok(Sos1Constraint {
            variables: self.variables.clone(),
            stage: Sos1SampledData {
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
                    "Cannot partially evaluate variable {:?} of SOS1 constraint. \
                     Fixing a SOS1 variable would change the constraint type.",
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

/// Check SOS1 feasibility for a single state.
///
/// Returns `(feasible, active_variable)`:
/// - feasible: at most one variable is non-zero
/// - active_variable: the variable that is non-zero (None if all zero or infeasible)
fn check_sos1(
    variables: &BTreeSet<VariableID>,
    state: &crate::v1::State,
    atol: ATol,
) -> crate::Result<(bool, Option<VariableID>)> {
    let mut active: Option<VariableID> = None;

    for &var_id in variables {
        let value = state.entries.get(&var_id.into_inner()).ok_or_else(|| {
            crate::error!(
                "Variable {:?} not found in state for SOS1 constraint",
                var_id,
            )
        })?;

        if value.abs() >= *atol {
            // Variable is non-zero
            if active.is_some() {
                // Multiple variables are non-zero → infeasible
                return Ok((false, None));
            }
            active = Some(var_id);
        }
    }

    // SOS1 allows all zeros (unlike one-hot)
    Ok((true, active))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Evaluate, Propagate, PropagateOutcome};
    use std::collections::HashMap;

    fn make_sos1(_id: u64, var_ids: &[u64]) -> Sos1Constraint {
        let vars = var_ids.iter().copied().map(VariableID::from).collect();
        Sos1Constraint::new(vars)
    }

    #[test]
    fn test_evaluate_feasible_one_nonzero() {
        let c = make_sos1(1, &[1, 2, 3]);
        // x1=0, x2=5.0, x3=0 → feasible, active=x2
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 5.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(result.stage.feasible);
        assert_eq!(result.stage.active_variable, Some(VariableID::from(2)));
    }

    #[test]
    fn test_evaluate_feasible_all_zeros() {
        let c = make_sos1(1, &[1, 2, 3]);
        // All zeros → feasible for SOS1 (unlike one-hot)
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 0.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(result.stage.feasible);
        assert_eq!(result.stage.active_variable, None);
    }

    #[test]
    fn test_evaluate_infeasible_multiple_nonzero() {
        let c = make_sos1(1, &[1, 2, 3]);
        // x1=1, x2=2, x3=0 → infeasible
        let state = crate::v1::State::from(HashMap::from([(1, 1.0), (2, 2.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(!result.stage.feasible);
        assert_eq!(result.stage.active_variable, None);
    }

    #[test]
    fn test_partial_evaluate_error() {
        let mut c = make_sos1(1, &[1, 2, 3]);
        let state = crate::v1::State::from(HashMap::from([(2, 1.0)]));
        let result = c.partial_evaluate(&state, ATol::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_partial_evaluate_no_overlap() {
        let mut c = make_sos1(1, &[1, 2, 3]);
        let state = crate::v1::State::from(HashMap::from([(99, 1.0)]));
        let result = c.partial_evaluate(&state, ATol::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_required_ids() {
        let c = make_sos1(1, &[1, 2, 3]);
        let ids = c.required_ids();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&VariableID::from(1)));
        assert!(ids.contains(&VariableID::from(2)));
        assert!(ids.contains(&VariableID::from(3)));
    }

    #[test]
    fn test_evaluate_samples() {
        let c = make_sos1(1, &[1, 2, 3]);

        let mut samples = crate::Sampled::<crate::v1::State>::default();
        // Sample 0: x1=0, x2=5.0, x3=0 → feasible, active=x2
        samples
            .append(
                [crate::SampleID::from(0)],
                crate::v1::State::from(HashMap::from([(1, 0.0), (2, 5.0), (3, 0.0)])),
            )
            .unwrap();
        // Sample 1: x1=1, x2=2, x3=0 → infeasible
        samples
            .append(
                [crate::SampleID::from(1)],
                crate::v1::State::from(HashMap::from([(1, 1.0), (2, 2.0), (3, 0.0)])),
            )
            .unwrap();
        // Sample 2: all zeros → feasible
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
        assert!(result.stage.feasible[&s2]);

        assert_eq!(result.stage.active_variable[&s0], Some(VariableID::from(2)));
        assert_eq!(result.stage.active_variable[&s1], None);
        assert_eq!(result.stage.active_variable[&s2], None);
    }

    // === Propagate tests ===

    #[test]
    fn test_propagate_nonzero_fixes_rest() {
        let c = make_sos1(1, &[1, 2, 3]);
        // x2=5.0 → Consumed, fix x1=0, x3=0
        let state = crate::v1::State::from(HashMap::from([(2, 5.0)]));
        let (outcome, additional) = c.propagate(&state, ATol::default()).unwrap();
        match outcome {
            PropagateOutcome::Consumed(original) => {
                assert_eq!(original.variables.len(), 3); // preserved
            }
            _ => panic!("Expected Consumed"),
        }
        assert_eq!(additional.entries.get(&1), Some(&0.0));
        assert_eq!(additional.entries.get(&3), Some(&0.0));
        assert_eq!(additional.entries.len(), 2);
    }

    #[test]
    fn test_propagate_zero_shrinks() {
        let c = make_sos1(1, &[1, 2, 3]);
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
    fn test_propagate_all_zeros_satisfied() {
        let c = make_sos1(1, &[1, 2, 3]);
        // All zeros → Consumed (vacuously satisfied)
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 0.0), (3, 0.0)]));
        let (outcome, additional) = c.propagate(&state, ATol::default()).unwrap();
        assert!(matches!(outcome, PropagateOutcome::Consumed(_)));
        assert!(additional.entries.is_empty());
    }

    #[test]
    fn test_propagate_multiple_nonzero_error() {
        let c = make_sos1(1, &[1, 2, 3]);
        let state = crate::v1::State::from(HashMap::from([(1, 1.0), (2, 2.0)]));
        let result = c.propagate(&state, ATol::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_propagate_no_overlap() {
        let c = make_sos1(1, &[1, 2, 3]);
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
