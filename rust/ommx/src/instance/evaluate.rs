use super::*;
use crate::{
    v1::{SampleSet, Solution},
    Evaluate,
};

impl Evaluate for Instance {
    type Output = Solution;
    type SampledOutput = SampleSet;

    fn evaluate(
        &self,
        solution: &v1::State,
    ) -> anyhow::Result<(Self::Output, std::collections::BTreeSet<u64>)> {
        todo!()
    }

    fn partial_evaluate(
        &mut self,
        state: &v1::State,
    ) -> anyhow::Result<std::collections::BTreeSet<u64>> {
        todo!()
    }

    fn required_ids(&self) -> std::collections::BTreeSet<u64> {
        todo!()
    }

    fn evaluate_samples(
        &self,
        samples: &v1::Samples,
    ) -> anyhow::Result<(Self::SampledOutput, std::collections::BTreeSet<u64>)> {
        todo!()
    }
}
