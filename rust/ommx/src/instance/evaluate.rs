use super::*;
use crate::{
    v1::{SampleSet, Solution},
    Evaluate,
};
use anyhow::Result;

impl Evaluate for Instance {
    type Output = Solution;
    type SampledOutput = SampleSet;

    fn evaluate(&self, solution: &v1::State) -> Result<Self::Output> {
        todo!()
    }

    fn partial_evaluate(&mut self, state: &v1::State) -> Result<()> {
        todo!()
    }

    fn required_ids(&self) -> std::collections::BTreeSet<u64> {
        todo!()
    }

    fn evaluate_samples(&self, samples: &v1::Samples) -> Result<Self::SampledOutput> {
        todo!()
    }
}
