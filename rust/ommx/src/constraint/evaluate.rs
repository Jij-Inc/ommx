use super::*;
use crate::{ATol, Evaluate, VariableIDSet};

impl Evaluate for Constraint<Created> {
    type Output = EvaluatedConstraint;
    type SampledOutput = SampledConstraint;

    fn evaluate(
        &self,
        solution: &crate::v1::State,
        atol: crate::ATol,
    ) -> anyhow::Result<Self::Output> {
        let evaluated_value = self.stage.function.evaluate(solution, atol)?;
        let used_decision_variable_ids = self.stage.function.required_ids();

        let feasible = match self.equality {
            Equality::EqualToZero => evaluated_value.abs() < *atol,
            Equality::LessThanOrEqualToZero => evaluated_value < *atol,
        };

        Ok(EvaluatedConstraint {
            id: self.id,
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
        atol: crate::ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        let evaluated_values_v1 = self.stage.function.evaluate_samples(samples, atol)?;
        let evaluated_values: crate::Sampled<f64> = evaluated_values_v1.try_into()?;

        let feasible: std::collections::BTreeMap<crate::SampleID, bool> = evaluated_values
            .iter()
            .map(|(sample_id, evaluated_value)| match self.equality {
                Equality::EqualToZero => (*sample_id, evaluated_value.abs() < *atol),
                Equality::LessThanOrEqualToZero => (*sample_id, *evaluated_value < *atol),
            })
            .collect();

        Ok(SampledConstraint {
            id: self.id,
            equality: self.equality,
            metadata: self.metadata.clone(),
            stage: SampledData {
                evaluated_values,
                dual_variables: None,
                feasible,
                used_decision_variable_ids: self.stage.function.required_ids(),
                removed_reason: None,
            },
        })
    }

    fn partial_evaluate(
        &mut self,
        state: &crate::v1::State,
        atol: crate::ATol,
    ) -> anyhow::Result<()> {
        self.stage.function.partial_evaluate(state, atol)
    }

    fn required_ids(&self) -> VariableIDSet {
        self.stage.function.required_ids()
    }
}

impl Evaluate for RemovedConstraint {
    type Output = EvaluatedConstraint;
    type SampledOutput = SampledConstraint;

    fn evaluate(&self, solution: &crate::v1::State, atol: ATol) -> anyhow::Result<Self::Output> {
        let evaluated_value = self.stage.function.evaluate(solution, atol)?;
        let used_decision_variable_ids = self.stage.function.required_ids();

        let feasible = match self.equality {
            Equality::EqualToZero => evaluated_value.abs() < *atol,
            Equality::LessThanOrEqualToZero => evaluated_value < *atol,
        };

        Ok(EvaluatedConstraint {
            id: self.id,
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

        let feasible: std::collections::BTreeMap<crate::SampleID, bool> = evaluated_values
            .iter()
            .map(|(sample_id, evaluated_value)| match self.equality {
                Equality::EqualToZero => (*sample_id, evaluated_value.abs() < *atol),
                Equality::LessThanOrEqualToZero => (*sample_id, *evaluated_value < *atol),
            })
            .collect();

        Ok(SampledConstraint {
            id: self.id,
            equality: self.equality,
            metadata: self.metadata.clone(),
            stage: SampledData {
                evaluated_values,
                dual_variables: None,
                feasible,
                used_decision_variable_ids: self.stage.function.required_ids(),
                removed_reason: Some(self.stage.removed_reason.clone()),
            },
        })
    }

    fn partial_evaluate(&mut self, state: &crate::v1::State, atol: ATol) -> anyhow::Result<()> {
        self.stage.function.partial_evaluate(state, atol)
    }

    fn required_ids(&self) -> VariableIDSet {
        self.stage.function.required_ids()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{constraint_type::SampledConstraintBehavior, random::*, v1::Samples};
    use proptest::prelude::*;

    fn constraint_and_samples() -> impl Strategy<Value = (Constraint<Created>, Samples)> {
        Constraint::arbitrary()
            .prop_flat_map(|c| {
                let ids = c.stage.function.required_ids();
                let state = arbitrary_state(ids);
                let samples = arbitrary_samples(SamplesParameters::default(), state);
                (Just(c), samples)
            })
            .boxed()
    }

    proptest! {
        #[test]
        fn test_evaluate_samples((c, samples) in constraint_and_samples()) {
            let evaluated = c.evaluate_samples(&samples, crate::ATol::default()).unwrap();
            let evaluated_each: FnvHashMap<u64, EvaluatedConstraint> = samples.iter().map(|(parameter_id, state)| {
                let value = c.evaluate(state, crate::ATol::default()).unwrap();
                (*parameter_id, value)
            }).collect();
            for (sample_id, each) in evaluated_each {
                let extracted = evaluated.get(SampleID::from(sample_id)).unwrap();
                prop_assert_eq!(extracted, each)
            }
        }
    }
}
