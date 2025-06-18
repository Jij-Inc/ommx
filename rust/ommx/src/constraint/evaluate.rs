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
        let used_decision_variable_ids = self
            .function
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect();
        
        let metadata = ConstraintMetadata {
            id: self.id,
            equality: self.equality,
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
            used_decision_variable_ids,
            removed_reason: None,
            removed_reason_parameters: FnvHashMap::default(),
        };
        
        let core = EvaluatedConstraintCore {
            evaluated_value,
            dual_variable: None,
        };
        
        Ok(EvaluatedConstraint { metadata, core })
    }

    fn evaluate_samples(
        &self,
        samples: &crate::v1::Samples,
        atol: crate::ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        let evaluated_values = self.function.evaluate_samples(samples, atol)?;
        
        // Convert v1::SampledValues to Sampled<EvaluatedConstraintCore>
        let sampled_values: crate::Sampled<f64> = evaluated_values.try_into()?;
        let cores = sampled_values.map(|evaluated_value| EvaluatedConstraintCore {
            evaluated_value,
            dual_variable: None,
        });
        
        let feasible: FnvHashMap<u64, bool> = cores
            .iter()
            .map(|(sample_id, core)| match self.equality {
                Equality::EqualToZero => (sample_id.into_inner(), core.evaluated_value.abs() < *atol),
                Equality::LessThanOrEqualToZero => (sample_id.into_inner(), core.evaluated_value < *atol),
            })
            .collect();
        
        let metadata = ConstraintMetadata {
            id: self.id,
            equality: self.equality,
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
            used_decision_variable_ids: self
                .function
                .required_ids()
                .into_iter()
                .map(|id| id.into_inner())
                .collect(),
            removed_reason: None,
            removed_reason_parameters: FnvHashMap::default(),
        };
        
        Ok(SampledConstraint {
            metadata,
            cores,
            feasible,
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
        let mut metadata = evaluated.metadata;
        metadata.removed_reason = Some(self.removed_reason.clone());
        metadata.removed_reason_parameters = self.removed_reason_parameters.clone();
        Ok(EvaluatedConstraint {
            metadata,
            core: evaluated.core,
        })
    }

    fn evaluate_samples(
        &self,
        samples: &crate::v1::Samples,
        atol: ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        let evaluated = self.constraint.evaluate_samples(samples, atol)?;
        let mut metadata = evaluated.metadata;
        metadata.removed_reason = Some(self.removed_reason.clone());
        metadata.removed_reason_parameters = self.removed_reason_parameters.clone();
        Ok(SampledConstraint {
            metadata,
            cores: evaluated.cores,
            feasible: evaluated.feasible,
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
                let extracted = evaluated.get(sample_id).unwrap();
                prop_assert_eq!(extracted, each)
            }
        }
    }
}
