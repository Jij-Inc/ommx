use super::*;
use crate::{
    v1::{SampleSet, Solution},
    Evaluate, VariableIDSet,
};
use anyhow::Result;

impl Evaluate for Instance {
    type Output = Solution;
    type SampledOutput = SampleSet;

    fn evaluate(&self, state: &v1::State, atol: f64) -> Result<Self::Output> {
        let state = self
            .analyze_decision_variables()
            .populate(state.clone(), atol)?;

        let objective = self.objective.evaluate(&state, atol)?;
        let evaluated_constraints = self
            .constraints
            .iter()
            .map(|(_id, constraint)| constraint.evaluate(&state, atol))
            .collect::<Result<Vec<_>>>()?;

        let decision_variables = self
            .decision_variables
            .values()
            .map(|dv| {
                let id = dv.id.into_inner();
                let value = state.entries.get(&id).unwrap(); // Safe unwrap, as we populate the state with the decision variables
                todo!()
            })
            .collect();

        Ok(Solution {
            state: Some(state.clone()),
            objective,
            evaluated_constraints,
            decision_variables,
            ..Default::default()
        })
    }

    fn evaluate_samples(&self, samples: &v1::Samples, atol: f64) -> Result<Self::SampledOutput> {
        todo!()
    }

    fn partial_evaluate(&mut self, state: &v1::State, atol: f64) -> Result<()> {
        todo!()
    }

    fn required_ids(&self) -> VariableIDSet {
        self.analyze_decision_variables().used().clone()
    }
}
