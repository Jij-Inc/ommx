use super::*;
use crate::{ATol, Evaluate, VariableIDSet};

impl Evaluate for IndicatorConstraint<Created> {
    type Output = EvaluatedIndicatorConstraint;
    type SampledOutput = SampledIndicatorConstraint;

    fn evaluate(&self, state: &crate::v1::State, atol: ATol) -> anyhow::Result<Self::Output> {
        let evaluated_value = self.stage.function.evaluate(state, atol)?;
        let used_decision_variable_ids = self.required_ids();

        // Check if indicator variable is ON (= 1)
        let indicator_on = state
            .entries
            .get(&self.indicator_variable.into_inner())
            .map_or(false, |v| *v > 1.0 - *atol);

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
            stage: EvaluatedData {
                evaluated_value,
                dual_variable: None,
                feasible,
                used_decision_variable_ids,
                removed_reason: None,
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

        let feasible: std::collections::BTreeMap<crate::SampleID, bool> = samples
            .iter()
            .map(|(sample_id, state)| {
                let indicator_on = state
                    .entries
                    .get(&self.indicator_variable.into_inner())
                    .map_or(false, |v| *v > 1.0 - *atol);

                let ev = evaluated_values
                    .get(crate::SampleID::from(*sample_id))
                    .copied()
                    .unwrap_or(0.0);

                let f = if indicator_on {
                    match self.equality {
                        Equality::EqualToZero => ev.abs() < *atol,
                        Equality::LessThanOrEqualToZero => ev < *atol,
                    }
                } else {
                    true
                };
                (crate::SampleID::from(*sample_id), f)
            })
            .collect();

        Ok(IndicatorConstraint {
            id: self.id,
            indicator_variable: self.indicator_variable,
            equality: self.equality,
            metadata: self.metadata.clone(),
            stage: SampledData {
                evaluated_values,
                dual_variables: None,
                feasible,
                used_decision_variable_ids: self.required_ids(),
                removed_reason: None,
            },
        })
    }

    fn partial_evaluate(&mut self, state: &crate::v1::State, atol: ATol) -> anyhow::Result<()> {
        self.stage.function.partial_evaluate(state, atol)
    }

    fn required_ids(&self) -> VariableIDSet {
        let mut ids = self.stage.function.required_ids();
        ids.insert(self.indicator_variable);
        ids
    }
}

impl Evaluate for RemovedIndicatorConstraint {
    type Output = EvaluatedIndicatorConstraint;
    type SampledOutput = SampledIndicatorConstraint;

    fn evaluate(&self, state: &crate::v1::State, atol: ATol) -> anyhow::Result<Self::Output> {
        let evaluated_value = self.stage.function.evaluate(state, atol)?;
        let used_decision_variable_ids = self.required_ids();

        let indicator_on = state
            .entries
            .get(&self.indicator_variable.into_inner())
            .map_or(false, |v| *v > 1.0 - *atol);

        let feasible = if indicator_on {
            match self.equality {
                Equality::EqualToZero => evaluated_value.abs() < *atol,
                Equality::LessThanOrEqualToZero => evaluated_value < *atol,
            }
        } else {
            true
        };

        Ok(IndicatorConstraint {
            id: self.id,
            indicator_variable: self.indicator_variable,
            equality: self.equality,
            metadata: self.metadata.clone(),
            stage: EvaluatedData {
                evaluated_value,
                dual_variable: None,
                feasible,
                used_decision_variable_ids,
                removed_reason: Some(self.stage.removed_reason.clone()),
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

        let feasible: std::collections::BTreeMap<crate::SampleID, bool> = samples
            .iter()
            .map(|(sample_id, state)| {
                let indicator_on = state
                    .entries
                    .get(&self.indicator_variable.into_inner())
                    .map_or(false, |v| *v > 1.0 - *atol);

                let ev = evaluated_values
                    .get(crate::SampleID::from(*sample_id))
                    .copied()
                    .unwrap_or(0.0);

                let f = if indicator_on {
                    match self.equality {
                        Equality::EqualToZero => ev.abs() < *atol,
                        Equality::LessThanOrEqualToZero => ev < *atol,
                    }
                } else {
                    true
                };
                (crate::SampleID::from(*sample_id), f)
            })
            .collect();

        Ok(IndicatorConstraint {
            id: self.id,
            indicator_variable: self.indicator_variable,
            equality: self.equality,
            metadata: self.metadata.clone(),
            stage: SampledData {
                evaluated_values,
                dual_variables: None,
                feasible,
                used_decision_variable_ids: self.required_ids(),
                removed_reason: Some(self.stage.removed_reason.clone()),
            },
        })
    }

    fn partial_evaluate(&mut self, state: &crate::v1::State, atol: ATol) -> anyhow::Result<()> {
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
    use crate::{coeff, linear, Evaluate, Function};
    use std::collections::HashMap;

    #[test]
    fn test_evaluate_indicator_on_feasible() {
        // x1 <= 5, indicator = x10
        let ic = IndicatorConstraint::new(
            ConstraintID::from(1),
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        // x1 = 3, x10 = 1 (indicator ON, 3 - 5 = -2 <= 0 → feasible)
        let state = crate::v1::State::from(HashMap::from([(1, 3.0), (10, 1.0)]));
        let result = ic.evaluate(&state, ATol::default()).unwrap();
        assert!(result.stage.feasible);
        assert_eq!(result.stage.evaluated_value, -2.0);
    }

    #[test]
    fn test_evaluate_indicator_on_infeasible() {
        // x1 <= 5, indicator = x10
        let ic = IndicatorConstraint::new(
            ConstraintID::from(1),
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        // x1 = 7, x10 = 1 (indicator ON, 7 - 5 = 2 > 0 → infeasible)
        let state = crate::v1::State::from(HashMap::from([(1, 7.0), (10, 1.0)]));
        let result = ic.evaluate(&state, ATol::default()).unwrap();
        assert!(!result.stage.feasible);
        assert_eq!(result.stage.evaluated_value, 2.0);
    }

    #[test]
    fn test_evaluate_indicator_off_always_feasible() {
        // x1 <= 5, indicator = x10
        let ic = IndicatorConstraint::new(
            ConstraintID::from(1),
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-5.0)),
        );

        // x1 = 100, x10 = 0 (indicator OFF → always feasible regardless of f(x))
        let state = crate::v1::State::from(HashMap::from([(1, 100.0), (10, 0.0)]));
        let result = ic.evaluate(&state, ATol::default()).unwrap();
        assert!(result.stage.feasible);
        assert_eq!(result.stage.evaluated_value, 95.0); // f(x) still evaluated for diagnostics
    }

    #[test]
    fn test_required_ids_includes_indicator() {
        let ic = IndicatorConstraint::new(
            ConstraintID::from(1),
            VariableID::from(10),
            Equality::EqualToZero,
            Function::from(linear!(1) + linear!(2)),
        );
        let ids = ic.required_ids();
        assert!(ids.contains(&VariableID::from(1)));
        assert!(ids.contains(&VariableID::from(2)));
        assert!(ids.contains(&VariableID::from(10))); // indicator variable
    }
}
