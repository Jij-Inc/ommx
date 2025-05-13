use super::*;
use crate::{
    v1::{EvaluatedConstraint, SampledConstraint},
    Evaluate, FnvHashMapExt,
};

impl Evaluate for Constraint {
    type Output = EvaluatedConstraint;
    type SampledOutput = SampledConstraint;

    fn evaluate(&self, solution: &crate::v1::State) -> anyhow::Result<Self::Output> {
        let evaluated_value = self.function.evaluate(solution)?;
        let used_decision_variable_ids = self.function.required_ids().into_iter().collect();
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
    ) -> anyhow::Result<Self::SampledOutput> {
        todo!()
    }

    fn partial_evaluate(&mut self, state: &crate::v1::State) -> anyhow::Result<()> {
        todo!()
    }

    fn required_ids(&self) -> std::collections::BTreeSet<u64> {
        self.function.required_ids()
    }
}
