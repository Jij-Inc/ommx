use super::*;
use crate::{Evaluate, VariableIDSet};

impl Evaluate for NamedFunction {
    type Output = EvaluatedNamedFunction;
    type SampledOutput = SampledNamedFunction;

    fn evaluate(
        &self,
        solution: &crate::v1::State,
        atol: crate::ATol,
    ) -> anyhow::Result<Self::Output> {
        let evaluated_value = self.function.evaluate(solution, atol)?;
        let used_decision_variable_ids = self.function.required_ids();
        Ok(EvaluatedNamedFunction {
            id: self.id,
            evaluated_value,
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
            used_decision_variable_ids,
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

    fn evaluate_samples(
        &self,
        samples: &crate::v1::Samples,
        atol: crate::ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        let evaluated_values_v1 = self.function.evaluate_samples(samples, atol)?;
        let evaluated_values = evaluated_values_v1.try_into()?;
        let used_decision_variable_ids = self.function.required_ids();
        Ok(SampledNamedFunction {
            id: self.id,
            evaluated_values,
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
            used_decision_variable_ids,
        })
    }
}
