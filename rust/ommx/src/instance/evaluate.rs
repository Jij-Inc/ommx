use super::*;
use crate::{
    v1::{SampleSet, Solution},
    Evaluate, VariableIDSet,
};

impl Evaluate for Instance {
    type Output = Solution;
    type SampledOutput = SampleSet;

    fn evaluate(&self, solution: &v1::State) -> anyhow::Result<Self::Output> {
        todo!()
    }

    fn evaluate_samples(&self, samples: &v1::Samples) -> anyhow::Result<Self::SampledOutput> {
        todo!()
    }

    fn partial_evaluate(&mut self, state: &v1::State) -> anyhow::Result<()> {
        todo!()
    }

    fn required_ids(&self) -> VariableIDSet {
        self.analyze_decision_variables().used()
    }
}
