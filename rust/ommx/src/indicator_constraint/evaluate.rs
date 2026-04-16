use super::*;
use crate::{ATol, Evaluate, Propagate, VariableIDSet};

impl Propagate for IndicatorConstraint<Created> {
    type Output = IndicatorPropagateOutput;

    fn propagate(
        mut self,
        state: &crate::v1::State,
        atol: ATol,
    ) -> anyhow::Result<(Self::Output, crate::v1::State)> {
        let empty_state = crate::v1::State::default();

        if let Some(&indicator_value) = state.entries.get(&self.indicator_variable.into_inner()) {
            if indicator_value > 1.0 - *atol {
                // Indicator ON → promote inner constraint to regular Constraint
                // Partial-evaluate the inner function first
                self.stage.function.partial_evaluate(state, atol)?;

                let constraint = crate::Constraint {
                    id: crate::ConstraintID::from(self.id.into_inner()),
                    equality: self.equality,
                    metadata: self.metadata,
                    stage: CreatedData {
                        function: self.stage.function,
                    },
                };
                Ok((IndicatorPropagateOutput::Promote(constraint), empty_state))
            } else if indicator_value.abs() < *atol {
                // Indicator OFF → vacuously satisfied
                Ok((IndicatorPropagateOutput::Removed, empty_state))
            } else {
                anyhow::bail!(
                    "Indicator variable {:?} of indicator constraint {:?} has invalid value {} (must be 0 or 1)",
                    self.indicator_variable,
                    self.id,
                    indicator_value
                );
            }
        } else {
            // Indicator variable not in state — partial-evaluate inner function only
            self.stage.function.partial_evaluate(state, atol)?;
            Ok((IndicatorPropagateOutput::Active(self), empty_state))
        }
    }
}

impl Evaluate for IndicatorConstraint<Created> {
    type Output = EvaluatedIndicatorConstraint;
    type SampledOutput = SampledIndicatorConstraint;

    fn evaluate(&self, state: &crate::v1::State, atol: ATol) -> anyhow::Result<Self::Output> {
        let evaluated_value = self.stage.function.evaluate(state, atol)?;
        let used_decision_variable_ids = self.required_ids();

        // Check indicator variable value
        let indicator_value = state
            .entries
            .get(&self.indicator_variable.into_inner())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Indicator variable {:?} not found in state for indicator constraint {:?}",
                    self.indicator_variable,
                    self.id
                )
            })?;

        let indicator_on = *indicator_value > 1.0 - *atol;

        let feasible = if indicator_on {
            // Indicator ON → check constraint as usual
            match self.equality {
                Equality::EqualToZero => evaluated_value.abs() < *atol,
                Equality::LessThanOrEqualToZero => evaluated_value < *atol,
            }
        } else {
            // Indicator OFF → always feasible
            true
        };

        Ok(IndicatorConstraint {
            id: self.id,
            indicator_variable: self.indicator_variable,
            equality: self.equality,
            metadata: self.metadata.clone(),
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
        samples: &crate::v1::Samples,
        atol: ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        let evaluated_values_v1 = self.stage.function.evaluate_samples(samples, atol)?;
        let evaluated_values: crate::Sampled<f64> = evaluated_values_v1.try_into()?;

        // Compute feasibility per sample.
        // We need both the evaluated value and the indicator variable's state,
        // so we iterate over samples (which provides the state) and look up the evaluated value.
        let mut feasible = std::collections::BTreeMap::new();
        let mut indicator_active = std::collections::BTreeMap::new();
        for (sample_id, state) in samples.iter() {
            let sample_id = crate::SampleID::from(*sample_id);
            let ev = *evaluated_values.get(sample_id)?;

            let indicator_value = state
                .entries
                .get(&self.indicator_variable.into_inner())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Indicator variable {:?} not found in sample {:?} for indicator constraint {:?}",
                        self.indicator_variable,
                        sample_id,
                        self.id
                    )
                })?;
            let indicator_on = *indicator_value > 1.0 - *atol;

            let f = if indicator_on {
                match self.equality {
                    Equality::EqualToZero => ev.abs() < *atol,
                    Equality::LessThanOrEqualToZero => ev < *atol,
                }
            } else {
                true
            };
            feasible.insert(sample_id, f);
            indicator_active.insert(sample_id, indicator_on);
        }

        Ok(IndicatorConstraint {
            id: self.id,
            indicator_variable: self.indicator_variable,
            equality: self.equality,
            metadata: self.metadata.clone(),
            stage: IndicatorSampledData {
                evaluated_values,
                feasible,
                indicator_active,
                used_decision_variable_ids: self.required_ids(),
            },
        })
    }

    fn partial_evaluate(&mut self, state: &crate::v1::State, atol: ATol) -> anyhow::Result<()> {
        if state
            .entries
            .contains_key(&self.indicator_variable.into_inner())
        {
            anyhow::bail!(
                "Cannot partially evaluate indicator variable {:?} of indicator constraint {:?}. \
                 Fixing an indicator variable would change the constraint type.",
                self.indicator_variable,
                self.id
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
    use crate::{coeff, linear, Evaluate, Function, Propagate};
    use std::collections::HashMap;

    #[test]
    fn test_evaluate_indicator_on_feasible() {
        // x1 <= 5, indicator = x10
        let ic = IndicatorConstraint::new(
            IndicatorConstraintID::from(1),
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
            IndicatorConstraintID::from(1),
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
            IndicatorConstraintID::from(1),
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
    fn test_required_ids_includes_indicator() {
        let ic = IndicatorConstraint::new(
            IndicatorConstraintID::from(1),
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
            IndicatorConstraintID::from(1),
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + linear!(2) + coeff!(-5.0)),
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
            IndicatorConstraintID::from(1),
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
            IndicatorConstraintID::from(1),
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        use crate::v1::samples::SamplesEntry;
        let samples = crate::v1::Samples {
            entries: vec![
                // Sample 0: x1=3, x10=1 → ON, feasible (3-5=-2 <= 0)
                SamplesEntry {
                    state: Some(crate::v1::State::from(HashMap::from([(1, 3.0), (10, 1.0)]))),
                    ids: vec![0],
                },
                // Sample 1: x1=7, x10=1 → ON, infeasible (7-5=2 > 0)
                SamplesEntry {
                    state: Some(crate::v1::State::from(HashMap::from([(1, 7.0), (10, 1.0)]))),
                    ids: vec![1],
                },
                // Sample 2: x1=100, x10=0 → OFF, feasible (always)
                SamplesEntry {
                    state: Some(crate::v1::State::from(HashMap::from([
                        (1, 100.0),
                        (10, 0.0),
                    ]))),
                    ids: vec![2],
                },
            ],
        };

        let result = ic.evaluate_samples(&samples, ATol::default()).unwrap();

        let s0 = crate::SampleID::from(0);
        let s1 = crate::SampleID::from(1);
        let s2 = crate::SampleID::from(2);

        // Feasibility
        assert_eq!(result.stage.feasible[&s0], true);
        assert_eq!(result.stage.feasible[&s1], false);
        assert_eq!(result.stage.feasible[&s2], true);

        // Indicator active
        assert_eq!(result.stage.indicator_active[&s0], true);
        assert_eq!(result.stage.indicator_active[&s1], true);
        assert_eq!(result.stage.indicator_active[&s2], false);
    }

    // === Propagate tests ===

    #[test]
    fn test_propagate_indicator_on_promotes() {
        // x1 <= 5, indicator = x10
        let ic = IndicatorConstraint::new(
            IndicatorConstraintID::from(1),
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        // x10 = 1 → promote inner constraint
        let state = crate::v1::State::from(HashMap::from([(10, 1.0)]));
        let (output, additional) = ic.propagate(&state, ATol::default()).unwrap();
        assert!(additional.entries.is_empty());
        match output {
            IndicatorPropagateOutput::Promote(constraint) => {
                assert_eq!(constraint.equality, Equality::LessThanOrEqualToZero);
                assert_eq!(constraint.id, crate::ConstraintID::from(1));
            }
            _ => panic!("Expected Promote"),
        }
    }

    #[test]
    fn test_propagate_indicator_off_removed() {
        let ic = IndicatorConstraint::new(
            IndicatorConstraintID::from(1),
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        // x10 = 0 → removed
        let state = crate::v1::State::from(HashMap::from([(10, 0.0)]));
        let (output, additional) = ic.propagate(&state, ATol::default()).unwrap();
        assert!(additional.entries.is_empty());
        assert!(matches!(output, IndicatorPropagateOutput::Removed));
    }

    #[test]
    fn test_propagate_indicator_not_fixed_partial_evaluates_function() {
        let ic = IndicatorConstraint::new(
            IndicatorConstraintID::from(1),
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + linear!(2) + coeff!(-5.0)),
        );

        // x1 = 3 (not indicator), x10 not in state → Active with partial-evaluated function
        let state = crate::v1::State::from(HashMap::from([(1, 3.0)]));
        let (output, additional) = ic.propagate(&state, ATol::default()).unwrap();
        assert!(additional.entries.is_empty());
        match output {
            IndicatorPropagateOutput::Active(ic) => {
                // x1 was substituted, only x2 remains in function
                let ids = ic.stage.function.required_ids();
                assert!(!ids.contains(&VariableID::from(1)));
                assert!(ids.contains(&VariableID::from(2)));
            }
            _ => panic!("Expected Active"),
        }
    }

    #[test]
    fn test_propagate_indicator_on_with_function_partial_eval() {
        // Both indicator fixed and function variables fixed
        let ic = IndicatorConstraint::new(
            IndicatorConstraintID::from(1),
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + linear!(2) + coeff!(-5.0)),
        );

        // x10=1, x1=3 → promote with x1 substituted in function
        let state = crate::v1::State::from(HashMap::from([(10, 1.0), (1, 3.0)]));
        let (output, additional) = ic.propagate(&state, ATol::default()).unwrap();
        assert!(additional.entries.is_empty());
        match output {
            IndicatorPropagateOutput::Promote(constraint) => {
                let ids = constraint.stage.function.required_ids();
                assert!(!ids.contains(&VariableID::from(1))); // substituted
                assert!(ids.contains(&VariableID::from(2))); // still free
            }
            _ => panic!("Expected Promote"),
        }
    }
}
