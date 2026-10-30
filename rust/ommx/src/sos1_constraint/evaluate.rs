use super::*;
use crate::EvaluatedConstraintBehavior;
use crate::{ATol, Evaluate, Propagate, PropagateOutcome, VariableIDSet};

fn ensure_sos1_value_is_finite(var_id: VariableID, value: f64) -> crate::Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        crate::bail!(
            "Variable {:?} in SOS1 constraint must be finite (value={})",
            var_id,
            value
        );
    }
}

impl Propagate for Sos1Constraint<Created> {
    type Transformed = std::convert::Infallible;

    fn propagate(
        mut self,
        state: &crate::v1::State,
        atol: ATol,
    ) -> crate::Result<(PropagateOutcome<Self>, crate::v1::State)> {
        let mut fixed_nonzero: Option<VariableID> = None;
        let mut unfixed = BTreeSet::new();
        let mut fixed_deviation = 0.0;

        for &var_id in &self.variables {
            let Some(&value) = state.entries.get(&var_id.into_inner()) else {
                unfixed.insert(var_id);
                continue;
            };

            ensure_sos1_value_is_finite(var_id, value)?;
            if atol.approx_is_zero(value) {
                fixed_deviation += value.abs();
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
            crate::ensure!(
                crate::constraint_type::violation_is_feasible(fixed_deviation, atol),
                "Fixed SOS1 values exceed the constraint tolerance: violation={fixed_deviation}"
            );
            // One variable is non-zero → constraint satisfied, fix remaining unfixed to 0
            let mut additional = crate::v1::State::default();
            for var_id in &unfixed {
                additional.entries.insert(var_id.into_inner(), 0.0);
            }
            Ok((PropagateOutcome::Consumed(self), additional))
        } else if unfixed.is_empty() {
            let evaluated = self.evaluate(state, atol)?;
            crate::ensure!(
                evaluated.is_feasible(atol),
                "Fixed SOS1 values exceed the constraint tolerance: violation={}",
                evaluated.violation()
            );
            Ok((
                PropagateOutcome::Consumed(self),
                crate::v1::State::default(),
            ))
        } else {
            // Multiple unfixed variables remain — modify and stay active
            // Keep approximate zeros: their contributions still belong to this
            // constraint's violation. Only exact zeros can be eliminated.
            self.variables
                .retain(|id| state.entries.get(&id.into_inner()) != Some(&0.0));
            Ok((PropagateOutcome::Active(self), crate::v1::State::default()))
        }
    }
}

impl Evaluate for Sos1Constraint<Created> {
    type Output = EvaluatedSos1Constraint;
    type SampledOutput = SampledSos1Constraint;

    fn evaluate(&self, state: &crate::v1::State, atol: ATol) -> crate::Result<Self::Output> {
        let used_decision_variable_ids = self.required_ids();
        let (violation, active_variable) =
            self.evaluate_members(|id| state.entries.get(&id.into_inner()).copied(), atol)?;

        Ok(Sos1Constraint {
            variables: self.variables.clone(),
            stage: Sos1EvaluatedData {
                activation_atol: atol,
                violation,
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
        let mut violations = crate::Sampled::default();
        let mut active_variable = BTreeMap::new();

        for (sample_id, state) in samples.iter() {
            let (violation, av) =
                self.evaluate_members(|id| state.entries.get(&id.into_inner()).copied(), atol)?;
            violations.append([*sample_id], violation)?;
            active_variable.insert(*sample_id, av);
        }

        Ok(Sos1Constraint {
            variables: self.variables.clone(),
            stage: Sos1SampledData {
                activation_atol: atol,
                violations,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Evaluate, Propagate, PropagateOutcome};
    use crate::{EvaluatedConstraintBehavior, SampledConstraintBehavior};
    use std::collections::HashMap;

    #[test]
    fn zero_classification_includes_the_atol_boundary() {
        let constraint = make_sos1(1, &[1, 2]);
        let atol = ATol::new(0.125).unwrap();
        let outside = f64::from_bits(0.125_f64.to_bits() + 1);

        let boundary = crate::v1::State::from(HashMap::from([(1, *atol), (2, 1.0)]));
        let evaluated = constraint.evaluate(&boundary, atol).unwrap();
        assert!(evaluated.is_feasible(atol));
        assert_eq!(evaluated.stage.active_variable, Some(VariableID::from(2)));

        let outside_state = crate::v1::State::from(HashMap::from([(1, outside), (2, 1.0)]));
        let mut samples = crate::Sampled::default();
        let boundary_sample_id = crate::SampleID::from(0);
        let outside_sample_id = crate::SampleID::from(1);
        samples
            .append([boundary_sample_id], boundary.clone())
            .unwrap();
        samples
            .append([outside_sample_id], outside_state.clone())
            .unwrap();
        let sampled = constraint.evaluate_samples(&samples, atol).unwrap();
        assert!(sampled.is_feasible_for(boundary_sample_id, atol).unwrap());
        assert_eq!(
            sampled.stage.active_variable[&boundary_sample_id],
            Some(VariableID::from(2))
        );
        assert!(!sampled.is_feasible_for(outside_sample_id, atol).unwrap());
        assert_eq!(sampled.stage.active_variable[&outside_sample_id], None);

        let (boundary_outcome, _) = constraint
            .clone()
            .propagate(&crate::v1::State::from(HashMap::from([(1, *atol)])), atol)
            .unwrap();
        assert!(matches!(boundary_outcome, PropagateOutcome::Active(_)));

        assert!(!constraint
            .evaluate(&outside_state, atol)
            .unwrap()
            .is_feasible(atol));
        let (outside_outcome, _) = constraint
            .propagate(&crate::v1::State::from(HashMap::from([(1, outside)])), atol)
            .unwrap();
        assert!(matches!(outside_outcome, PropagateOutcome::Consumed(_)));
    }

    fn make_sos1(_id: u64, var_ids: &[u64]) -> Sos1Constraint {
        let vars = var_ids.iter().copied().map(VariableID::from).collect();
        Sos1Constraint::new(vars).unwrap()
    }

    #[test]
    fn test_evaluate_feasible_one_nonzero() {
        let c = make_sos1(1, &[1, 2, 3]);
        // x1=0, x2=5.0, x3=0 → feasible, active=x2
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 5.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(result.is_feasible(crate::ATol::default()));
        assert_eq!(result.stage.active_variable, Some(VariableID::from(2)));
    }

    #[test]
    fn test_evaluate_feasible_all_zeros() {
        let c = make_sos1(1, &[1, 2, 3]);
        // All zeros → feasible for SOS1 (unlike one-hot)
        let state = crate::v1::State::from(HashMap::from([(1, 0.0), (2, 0.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(result.is_feasible(crate::ATol::default()));
        assert_eq!(result.stage.active_variable, None);
    }

    #[test]
    fn test_evaluate_infeasible_multiple_nonzero() {
        let c = make_sos1(1, &[1, 2, 3]);
        // x1=1, x2=2, x3=0 → infeasible
        let state = crate::v1::State::from(HashMap::from([(1, 1.0), (2, 2.0), (3, 0.0)]));
        let result = c.evaluate(&state, ATol::default()).unwrap();
        assert!(!result.is_feasible(crate::ATol::default()));
        assert_eq!(result.stage.active_variable, None);
    }

    #[test]
    fn test_evaluate_rejects_non_finite_value() {
        let c = make_sos1(1, &[1, 2, 3]);
        let state = crate::v1::State::from(HashMap::from([(1, f64::NAN), (2, 0.0), (3, 0.0)]));

        let err = c.evaluate(&state, ATol::default()).unwrap_err();
        assert!(err.to_string().contains("must be finite"));
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

        assert!(result.is_feasible_for(s0, crate::ATol::default()).unwrap());
        assert!(!result.is_feasible_for(s1, crate::ATol::default()).unwrap());
        assert!(result.is_feasible_for(s2, crate::ATol::default()).unwrap());

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
    fn test_propagate_rejects_non_finite_value() {
        let c = make_sos1(1, &[1, 2, 3]);
        let state = crate::v1::State::from(HashMap::from([(1, f64::INFINITY)]));

        let err = c.propagate(&state, ATol::default()).unwrap_err();
        assert!(err.to_string().contains("must be finite"));
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
