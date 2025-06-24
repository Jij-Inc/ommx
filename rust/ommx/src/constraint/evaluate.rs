use super::*;
use crate::{ATol, Evaluate, VariableIDSet};
use fnv::FnvHashMap;

impl Evaluate for Constraint {
    type Output = EvaluatedConstraint;
    type SampledOutput = SampledConstraint;

    fn evaluate(
        &self,
        solution: &crate::v1::State,
        atol: crate::ATol,
    ) -> anyhow::Result<Self::Output> {
        let evaluated_value = self.function.evaluate(solution, atol)?;
        let used_decision_variable_ids = self.function.required_ids();

        let metadata = ConstraintMetadata {
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
        };

        let feasible = match self.equality {
            Equality::EqualToZero => evaluated_value.abs() < *atol,
            Equality::LessThanOrEqualToZero => evaluated_value < *atol,
        };

        Ok(EvaluatedConstraint {
            id: self.id,
            equality: self.equality,
            metadata,
            evaluated_value,
            dual_variable: None,
            feasible,
            used_decision_variable_ids,
            removed_reason: None,
            removed_reason_parameters: FnvHashMap::default(),
        })
    }

    fn evaluate_samples(
        &self,
        samples: &crate::v1::Samples,
        atol: crate::ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        let evaluated_values_v1 = self.function.evaluate_samples(samples, atol)?;

        // Convert v1::SampledValues to Sampled<f64>
        let evaluated_values: crate::Sampled<f64> = evaluated_values_v1.try_into()?;

        let feasible: std::collections::BTreeMap<crate::SampleID, bool> = evaluated_values
            .iter()
            .map(|(sample_id, evaluated_value)| match self.equality {
                Equality::EqualToZero => (*sample_id, evaluated_value.abs() < *atol),
                Equality::LessThanOrEqualToZero => (*sample_id, *evaluated_value < *atol),
            })
            .collect();

        let metadata = ConstraintMetadata {
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
        };

        Ok(SampledConstraint {
            id: self.id,
            equality: self.equality,
            metadata,
            evaluated_values,
            dual_variables: None, // TODO: Support dual variables in the future
            feasible,
            used_decision_variable_ids: self.function.required_ids(),
            removed_reason: None,
            removed_reason_parameters: FnvHashMap::default(),
        })
    }

    fn partial_evaluate(
        &mut self,
        state: &crate::v1::State,
        atol: crate::ATol,
    ) -> anyhow::Result<()> {
        self.function.partial_evaluate(state, atol)
    }

    fn required_ids(&self) -> VariableIDSet {
        self.function.required_ids()
    }
}

impl Evaluate for RemovedConstraint {
    type Output = EvaluatedConstraint;
    type SampledOutput = SampledConstraint;

    fn evaluate(&self, solution: &crate::v1::State, atol: ATol) -> anyhow::Result<Self::Output> {
        let evaluated = self.constraint.evaluate(solution, atol)?;
        Ok(EvaluatedConstraint {
            id: evaluated.id,
            equality: evaluated.equality,
            metadata: evaluated.metadata,
            evaluated_value: evaluated.evaluated_value,
            dual_variable: evaluated.dual_variable,
            feasible: evaluated.feasible,
            used_decision_variable_ids: evaluated.used_decision_variable_ids,
            removed_reason: Some(self.removed_reason.clone()),
            removed_reason_parameters: self.removed_reason_parameters.clone(),
        })
    }

    fn evaluate_samples(
        &self,
        samples: &crate::v1::Samples,
        atol: ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        let evaluated = self.constraint.evaluate_samples(samples, atol)?;
        Ok(SampledConstraint {
            id: evaluated.id,
            equality: evaluated.equality,
            metadata: evaluated.metadata,
            evaluated_values: evaluated.evaluated_values,
            dual_variables: evaluated.dual_variables,
            feasible: evaluated.feasible,
            used_decision_variable_ids: evaluated.used_decision_variable_ids,
            removed_reason: Some(self.removed_reason.clone()),
            removed_reason_parameters: self.removed_reason_parameters.clone(),
        })
    }

    fn partial_evaluate(&mut self, state: &crate::v1::State, atol: ATol) -> anyhow::Result<()> {
        self.constraint.partial_evaluate(state, atol)
    }

    fn required_ids(&self) -> VariableIDSet {
        self.constraint.required_ids()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{random::*, v1::Samples};
    use proptest::prelude::*;

    fn constraint_and_samples() -> impl Strategy<Value = (Constraint, Samples)> {
        Constraint::arbitrary()
            .prop_flat_map(|c| {
                let ids = c.function.required_ids();
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
