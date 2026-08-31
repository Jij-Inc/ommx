use super::*;
use crate::{ATol, Constraint, CreatedData, Evaluate, Propagate, PropagateOutcome, VariableIDSet};
use anyhow::Context;

// Full evaluation leaves ordinary kind and bound violations to Solution
// feasibility. Indicator activation is structural, however, so its value must
// be interpretable as binary before the constraint can be evaluated.
fn indicator_is_active(
    indicator_variable: VariableID,
    value: f64,
    sample_id: Option<SampleID>,
    atol: ATol,
) -> crate::Result<bool> {
    crate::DecisionVariable::binary()
        .check_value_consistency(indicator_variable, value, atol)
        .with_context(|| match sample_id {
            Some(sample_id) => format!(
                "indicator variable {indicator_variable:?} has an invalid activation value in sample {sample_id:?}"
            ),
            None => format!(
                "indicator variable {indicator_variable:?} has an invalid activation value"
            ),
        })?;
    Ok(atol.approx_eq(value, 1.0))
}

impl Propagate for IndicatorConstraint<Created> {
    type Transformed = Constraint<Created>;

    fn propagate(
        mut self,
        state: &crate::v1::State,
        atol: ATol,
    ) -> crate::Result<(PropagateOutcome<Self>, crate::v1::State)> {
        let empty_state = crate::v1::State::default();

        if let Some(&indicator_value) = state.entries.get(&self.indicator_variable.into_inner()) {
            if indicator_is_active(self.indicator_variable, indicator_value, None, atol)? {
                // Indicator ON (~1) → promote inner constraint to regular Constraint.
                // Clone the function so self (going to removed) retains its data.
                let mut promoted_function = self.stage.function.clone();
                promoted_function.partial_evaluate(state, atol)?;

                // Provenance is added by the caller that has the original IndicatorConstraintID.
                // NOTE: per-element constraint context is gone in v3; the caller is
                // responsible for moving the IndicatorConstraint's context
                // (looked up via the IndicatorConstraintCollection's
                // ConstraintContextStore) into the regular ConstraintCollection's
                // store under the new id at insertion time.
                let new = Constraint {
                    equality: self.equality,
                    stage: CreatedData {
                        function: promoted_function,
                    },
                };
                Ok((
                    PropagateOutcome::Transformed {
                        original: self,
                        new,
                    },
                    empty_state,
                ))
            } else {
                // Indicator OFF (~0) → vacuously satisfied; the constraint is consumed.
                Ok((PropagateOutcome::Consumed(self), empty_state))
            }
        } else {
            // Indicator variable not in state — partial-evaluate inner function in-place
            self.stage.function.partial_evaluate(state, atol)?;
            Ok((PropagateOutcome::Active(self), empty_state))
        }
    }
}

impl Evaluate for IndicatorConstraint<Created> {
    type Output = EvaluatedIndicatorConstraint;
    type SampledOutput = SampledIndicatorConstraint;

    fn evaluate(&self, state: &crate::v1::State, atol: ATol) -> crate::Result<Self::Output> {
        let evaluated_value = self.stage.function.evaluate(state, atol)?;
        let used_decision_variable_ids = self.required_ids();

        // Check indicator variable value
        let indicator_value = state
            .entries
            .get(&self.indicator_variable.into_inner())
            .ok_or_else(|| {
                crate::error!(
                    "Indicator variable {:?} not found in state for indicator constraint",
                    self.indicator_variable,
                )
            })?;

        let indicator_on =
            indicator_is_active(self.indicator_variable, *indicator_value, None, atol)?;

        let feasible = if indicator_on {
            // Indicator ON → check constraint as usual
            self.equality.is_satisfied(evaluated_value, atol)
        } else {
            // Indicator OFF → always feasible
            true
        };

        Ok(IndicatorConstraint {
            indicator_variable: self.indicator_variable,
            equality: self.equality,
            stage: IndicatorEvaluatedData {
                evaluated_value,
                feasible,
                indicator_active: indicator_on,
                used_decision_variable_ids,
            },
        })
    }

    fn evaluate_samples(
        &self,
        samples: &crate::Sampled<crate::v1::State>,
        atol: ATol,
    ) -> crate::Result<Self::SampledOutput> {
        let evaluated_values = self.stage.function.evaluate_samples(samples, atol)?;

        // Compute feasibility per sample.
        // We need both the evaluated value and the indicator variable's state,
        // so we iterate over samples (which provides the state) and look up the evaluated value.
        let mut feasible = std::collections::BTreeMap::new();
        let mut indicator_active = std::collections::BTreeMap::new();
        for (sample_id, state) in samples.iter() {
            let sample_id = *sample_id;
            let ev = *evaluated_values.get(sample_id).ok_or_else(|| {
                crate::error!(
                    "Sample ID {sample_id:?} missing from evaluated values during indicator-constraint evaluation"
                )
            })?;

            let indicator_value = state
                .entries
                .get(&self.indicator_variable.into_inner())
                .ok_or_else(|| {
                    crate::error!(
                        "Indicator variable {:?} not found in sample {:?} for indicator constraint",
                        self.indicator_variable,
                        sample_id,
                    )
                })?;
            let indicator_on = indicator_is_active(
                self.indicator_variable,
                *indicator_value,
                Some(sample_id),
                atol,
            )?;

            let f = if indicator_on {
                self.equality.is_satisfied(ev, atol)
            } else {
                true
            };
            feasible.insert(sample_id, f);
            indicator_active.insert(sample_id, indicator_on);
        }

        Ok(IndicatorConstraint {
            indicator_variable: self.indicator_variable,
            equality: self.equality,
            stage: IndicatorSampledData {
                evaluated_values,
                feasible,
                indicator_active,
                used_decision_variable_ids: self.required_ids(),
            },
        })
    }

    fn partial_evaluate(&mut self, state: &crate::v1::State, atol: ATol) -> crate::Result<()> {
        if state
            .entries
            .contains_key(&self.indicator_variable.into_inner())
        {
            crate::bail!(
                "Cannot partially evaluate indicator variable {:?} of indicator constraint. \
                 Fixing an indicator variable would change the constraint type.",
                self.indicator_variable,
            );
        }
        self.stage.function.partial_evaluate(state, atol)
    }

    fn required_ids(&self) -> VariableIDSet {
        let mut ids = self.stage.function.required_ids();
        ids.insert(self.indicator_variable);
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, Evaluate, Function, Propagate, PropagateOutcome};
    use std::collections::HashMap;

    #[test]
    fn activation_and_residual_include_the_atol_boundary() {
        let atol = ATol::new(0.125).unwrap();
        let outside = f64::from_bits(0.125_f64.to_bits() + 1);
        let indicator_boundary = 1.0 + *atol;
        let indicator_id = VariableID::from(10);
        let boundary_constraint = IndicatorConstraint::new(
            indicator_id,
            Equality::EqualToZero,
            Function::Constant(crate::Coefficient::try_from(*atol).unwrap()),
        );
        let boundary_state = crate::v1::State::from(HashMap::from([(10, indicator_boundary)]));

        let evaluated = boundary_constraint.evaluate(&boundary_state, atol).unwrap();
        assert!(evaluated.stage.indicator_active);
        assert!(evaluated.stage.feasible);

        let sample_id = crate::SampleID::from(7);
        let sampled = boundary_constraint
            .evaluate_samples(
                &crate::Sampled::from((sample_id, boundary_state.clone())),
                atol,
            )
            .unwrap();
        assert_eq!(sampled.stage.indicator_active.get(&sample_id), Some(&true));
        assert_eq!(sampled.stage.feasible.get(&sample_id), Some(&true));

        let (outcome, _) = boundary_constraint
            .clone()
            .propagate(&boundary_state, atol)
            .unwrap();
        assert!(matches!(outcome, PropagateOutcome::Transformed { .. }));

        let outside_constraint = IndicatorConstraint::new(
            indicator_id,
            Equality::EqualToZero,
            Function::Constant(crate::Coefficient::try_from(outside).unwrap()),
        );
        let exact_on = crate::v1::State::from(HashMap::from([(10, 1.0)]));
        assert!(
            !outside_constraint
                .evaluate(&exact_on, atol)
                .unwrap()
                .stage
                .feasible
        );

        let outside_indicator = f64::from_bits(indicator_boundary.to_bits() + 1);
        let invalid_state = crate::v1::State::from(HashMap::from([(10, outside_indicator)]));
        assert!(boundary_constraint.evaluate(&invalid_state, atol).is_err());
    }

    #[test]
    fn test_evaluate_indicator_on_feasible() {
        // x1 <= 5, indicator = x10
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        // x1 = 3, x10 = 1 (indicator ON, 3 - 5 = -2 <= 0 → feasible)
        let state = crate::v1::State::from(HashMap::from([(1, 3.0), (10, 1.0)]));
        let result = ic.evaluate(&state, ATol::default()).unwrap();
        assert!(result.stage.feasible);
        assert!(result.stage.indicator_active);
        assert_eq!(result.stage.evaluated_value, -2.0);
    }

    #[test]
    fn test_evaluate_indicator_on_infeasible() {
        // x1 <= 5, indicator = x10
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        // x1 = 7, x10 = 1 (indicator ON, 7 - 5 = 2 > 0 → infeasible)
        let state = crate::v1::State::from(HashMap::from([(1, 7.0), (10, 1.0)]));
        let result = ic.evaluate(&state, ATol::default()).unwrap();
        assert!(!result.stage.feasible);
        assert!(result.stage.indicator_active);
        assert_eq!(result.stage.evaluated_value, 2.0);
    }

    #[test]
    fn test_evaluate_indicator_off_always_feasible() {
        // x1 <= 5, indicator = x10
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        // x1 = 100, x10 = 0 (indicator OFF → always feasible regardless of f(x))
        let state = crate::v1::State::from(HashMap::from([(1, 100.0), (10, 0.0)]));
        let result = ic.evaluate(&state, ATol::default()).unwrap();
        assert!(result.stage.feasible);
        assert!(!result.stage.indicator_active);
        assert_eq!(result.stage.evaluated_value, 95.0); // f(x) still evaluated for diagnostics
    }

    #[test]
    fn test_evaluate_invalid_indicator_value_preserves_decision_variable_signal() {
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1)),
        );
        let state = crate::v1::State::from(HashMap::from([(1, 3.0), (10, 0.5)]));

        let error = ic.evaluate(&state, ATol::default()).unwrap_err();
        let signal = error
            .downcast_ref::<crate::DecisionVariableError>()
            .expect("invalid caller-owned indicator value must remain downcastable");
        assert!(matches!(
            signal,
            crate::DecisionVariableError::SubstitutedValueInconsistent {
                id,
                kind: crate::Kind::Binary,
                substituted_value,
                ..
            } if *id == VariableID::from(10) && *substituted_value == 0.5
        ));
    }

    #[test]
    fn test_required_ids_includes_indicator() {
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::EqualToZero,
            Function::from(linear!(1) + linear!(2)),
        );
        let ids = ic.required_ids();
        assert!(ids.contains(&VariableID::from(1)));
        assert!(ids.contains(&VariableID::from(2)));
        assert!(ids.contains(&VariableID::from(10))); // indicator variable
    }

    #[test]
    fn test_partial_evaluate_function_variable() {
        // Partial evaluate a variable in the function should work
        let mut ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(((linear!(1) + linear!(2)).unwrap() + coeff!(-5.0)).unwrap()),
        );

        // Fix x1 = 3, but leave x2 and indicator x10 free
        let state = crate::v1::State::from(HashMap::from([(1, 3.0)]));
        ic.partial_evaluate(&state, ATol::default()).unwrap();

        // Function should now only depend on x2
        let ids = ic.stage.function.required_ids();
        assert!(!ids.contains(&VariableID::from(1)));
        assert!(ids.contains(&VariableID::from(2)));
    }

    #[test]
    fn test_partial_evaluate_indicator_variable_fails() {
        // Partial evaluate the indicator variable itself should fail
        let mut ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        // Try to fix x10 (indicator variable)
        let state = crate::v1::State::from(HashMap::from([(10, 1.0)]));
        let result = ic.partial_evaluate(&state, ATol::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_samples_indicator() {
        // x1 <= 5, indicator = x10
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        let mut samples = crate::Sampled::<crate::v1::State>::default();
        // Sample 0: x1=3, x10=1 → ON, feasible (3-5=-2 <= 0)
        samples
            .append(
                [crate::SampleID::from(0)],
                crate::v1::State::from(HashMap::from([(1, 3.0), (10, 1.0)])),
            )
            .unwrap();
        // Sample 1: x1=7, x10=1 → ON, infeasible (7-5=2 > 0)
        samples
            .append(
                [crate::SampleID::from(1)],
                crate::v1::State::from(HashMap::from([(1, 7.0), (10, 1.0)])),
            )
            .unwrap();
        // Sample 2: x1=100, x10=0 → OFF, feasible (always)
        samples
            .append(
                [crate::SampleID::from(2)],
                crate::v1::State::from(HashMap::from([(1, 100.0), (10, 0.0)])),
            )
            .unwrap();

        let result = ic.evaluate_samples(&samples, ATol::default()).unwrap();

        let s0 = crate::SampleID::from(0);
        let s1 = crate::SampleID::from(1);
        let s2 = crate::SampleID::from(2);

        // Feasibility
        assert!(result.stage.feasible[&s0]);
        assert!(!result.stage.feasible[&s1]);
        assert!(result.stage.feasible[&s2]);

        // Indicator active
        assert!(result.stage.indicator_active[&s0]);
        assert!(result.stage.indicator_active[&s1]);
        assert!(!result.stage.indicator_active[&s2]);
    }

    #[test]
    fn test_evaluate_samples_invalid_indicator_value_preserves_decision_variable_signal() {
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1)),
        );
        let sample_id = crate::SampleID::from(7);
        let samples = crate::Sampled::from((
            sample_id,
            crate::v1::State::from(HashMap::from([(1, 3.0), (10, 0.5)])),
        ));

        let error = ic.evaluate_samples(&samples, ATol::default()).unwrap_err();
        assert!(error.to_string().contains("sample SampleID(7)"));
        let signal = error
            .downcast_ref::<crate::DecisionVariableError>()
            .expect("invalid sampled indicator value must remain downcastable");
        assert!(matches!(
            signal,
            crate::DecisionVariableError::SubstitutedValueInconsistent {
                id,
                kind: crate::Kind::Binary,
                substituted_value,
                ..
            } if *id == VariableID::from(10) && *substituted_value == 0.5
        ));
    }

    // === Propagate tests ===

    #[test]
    fn test_propagate_indicator_on_promotes() {
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        // x10 = 1 → Transformed: promote inner constraint
        let state = crate::v1::State::from(HashMap::from([(10, 1.0)]));
        let (outcome, additional) = ic.propagate(&state, ATol::default()).unwrap();
        assert!(additional.entries.is_empty());
        match outcome {
            PropagateOutcome::Transformed { original, new } => {
                assert_eq!(new.equality, Equality::LessThanOrEqualToZero);
                // Per-element constraint context is gone in v3; provenance and
                // modeling labels are added by the caller (Instance) into its
                // ConstraintContextStore at the collection level.
                // Original indicator constraint preserved for removed set
                assert_eq!(original.indicator_variable, VariableID::from(10));
            }
            _ => panic!("Expected Transformed"),
        }
    }

    #[test]
    fn test_propagate_indicator_off_consumed() {
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        // x10 = 0 → Consumed (vacuously satisfied)
        let state = crate::v1::State::from(HashMap::from([(10, 0.0)]));
        let (outcome, additional) = ic.propagate(&state, ATol::default()).unwrap();
        assert!(additional.entries.is_empty());
        assert!(matches!(outcome, PropagateOutcome::Consumed(_)));
    }

    #[test]
    fn test_propagate_invalid_indicator_value_preserves_decision_variable_signal() {
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1)),
        );
        let state = crate::v1::State::from(HashMap::from([(10, 0.5)]));

        let error = ic.propagate(&state, ATol::default()).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<crate::DecisionVariableError>(),
            Some(
                crate::DecisionVariableError::SubstitutedValueInconsistent {
                    id,
                    kind: crate::Kind::Binary,
                    substituted_value,
                    ..
                }
            ) if *id == VariableID::from(10) && *substituted_value == 0.5
        ));
    }

    #[test]
    fn test_propagate_indicator_not_fixed_partial_evaluates_function() {
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(((linear!(1) + linear!(2)).unwrap() + coeff!(-5.0)).unwrap()),
        );

        // x1 = 3 (not indicator) → Active: function partial-evaluated
        let state = crate::v1::State::from(HashMap::from([(1, 3.0)]));
        let (outcome, additional) = ic.propagate(&state, ATol::default()).unwrap();
        assert!(additional.entries.is_empty());
        match outcome {
            PropagateOutcome::Active(ic) => {
                let ids = ic.stage.function.required_ids();
                assert!(!ids.contains(&VariableID::from(1)));
                assert!(ids.contains(&VariableID::from(2)));
            }
            _ => panic!("Expected Active"),
        }
    }

    #[test]
    fn test_propagate_indicator_on_with_function_partial_eval() {
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(((linear!(1) + linear!(2)).unwrap() + coeff!(-5.0)).unwrap()),
        );

        // x10=1, x1=3 → Transformed with x1 substituted in promoted function
        let state = crate::v1::State::from(HashMap::from([(10, 1.0), (1, 3.0)]));
        let (outcome, additional) = ic.propagate(&state, ATol::default()).unwrap();
        assert!(additional.entries.is_empty());
        match outcome {
            PropagateOutcome::Transformed { original, new } => {
                let ids = new.function().required_ids();
                assert!(!ids.contains(&VariableID::from(1))); // substituted
                assert!(ids.contains(&VariableID::from(2))); // still free
                                                             // Original ic still has unmodified function (was cloned for promotion)
                assert!(original
                    .stage
                    .function
                    .required_ids()
                    .contains(&VariableID::from(1)));
            }
            _ => panic!("Expected Transformed"),
        }
    }
}
