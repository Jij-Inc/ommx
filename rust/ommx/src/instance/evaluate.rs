use super::*;
use crate::{
    v1::{SampleSet, Solution},
    Evaluate, VariableIDSet,
};
use anyhow::{bail, Result};

impl Evaluate for Instance {
    type Output = Solution;
    type SampledOutput = SampleSet;

    fn evaluate(&self, state: &v1::State) -> Result<Self::Output> {
        // Use required IDs to evaluate the objective and constraints
        let objective = self.objective.evaluate(state)?;
        let evaluated_constraints = self
            .constraints
            .iter()
            .map(|(_id, constraint)| constraint.evaluate(state))
            .collect::<Result<Vec<_>>>()?;

        let mut state = state.clone();
        let analysis = self.analyze_decision_variables();
        // Check fixed variables are consistent
        for (id, value) in analysis.fixed() {
            if let Some(v) = state.entries.get(id) {
                if (v - value).abs() > 1e-6 {
                    bail!("Inconsistent fixed variable: {id} = {value}, but found {v} in state");
                }
            } else {
                // TODO: Check bound
                state.entries.insert(id.into_inner(), *value);
            }
        }
        // TODO: Fix a possible value for irrelevant variables
        for id in analysis.irrelevant() {
            todo!()
        }
        // TODO: Fill dependent variables

        Ok(Solution {
            state: Some(state.clone()),
            objective,
            evaluated_constraints,
            ..Default::default()
        })
    }

    fn evaluate_samples(&self, samples: &v1::Samples) -> Result<Self::SampledOutput> {
        todo!()
    }

    fn partial_evaluate(&mut self, state: &v1::State) -> Result<()> {
        todo!()
    }

    fn required_ids(&self) -> VariableIDSet {
        self.analyze_decision_variables().used()
    }
}
