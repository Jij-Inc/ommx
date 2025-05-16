use super::*;
use crate::{
    v1::{EvaluatedConstraint, SampledConstraint},
    Evaluate, FnvHashMapExt, VariableIDSet,
};
use std::collections::HashMap;

impl Evaluate for Constraint {
    type Output = EvaluatedConstraint;
    type SampledOutput = SampledConstraint;

    fn evaluate(&self, solution: &crate::v1::State, atol: f64) -> anyhow::Result<Self::Output> {
        let evaluated_value = self.function.evaluate(solution, atol)?;
        let used_decision_variable_ids = self
            .function
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect();
        Ok(EvaluatedConstraint {
            id: self.id.into_inner(),
            equality: self.equality.into(),
            evaluated_value,
            used_decision_variable_ids,
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.to_std(),
            description: self.description.clone(),
            dual_variable: None,
            removed_reason: None,
            removed_reason_parameters: Default::default(),
        })
    }

    fn evaluate_samples(
        &self,
        samples: &crate::v1::Samples,
        atol: f64,
    ) -> anyhow::Result<Self::SampledOutput> {
        let evaluated_values = self.function.evaluate_samples(samples, atol)?;
        let feasible: HashMap<u64, bool> = evaluated_values
            .iter()
            .map(|(sample_id, value)| match self.equality {
                Equality::EqualToZero => (*sample_id, value.abs() < 1e-6),
                Equality::LessThanOrEqualToZero => (*sample_id, *value < 1e-6),
            })
            .collect();
        Ok(SampledConstraint {
            id: self.id.into_inner(),
            evaluated_values: Some(evaluated_values),
            used_decision_variable_ids: self
                .function
                .required_ids()
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.to_std(),
            description: self.description.clone(),
            equality: self.equality.into(),
            feasible,
            removed_reason: None,
            removed_reason_parameters: Default::default(),
        })
    }

    fn partial_evaluate(&mut self, state: &crate::v1::State, atol: f64) -> anyhow::Result<()> {
        self.function.partial_evaluate(state, atol)
    }

    fn required_ids(&self) -> VariableIDSet {
        self.function.required_ids()
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
            let evaluated = c.evaluate_samples(&samples, 1e-6).unwrap();
            let evaluated_each: FnvHashMap<u64, EvaluatedConstraint> = samples.iter().map(|(parameter_id, state)| {
                let value = c.evaluate(state, 1e-6).unwrap();
                (*parameter_id, value)
            }).collect();
            for (sample_id, each) in evaluated_each {
                let extracted = evaluated.get(sample_id).unwrap();
                prop_assert_eq!(extracted, each)
            }
        }
    }
}
