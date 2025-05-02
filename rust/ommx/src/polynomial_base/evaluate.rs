use super::*;
use crate::{
    v1::{SampledValues, Samples, State},
    Evaluate,
};
use anyhow::Result;
use std::collections::BTreeSet;

impl<M: Monomial> Evaluate for PolynomialBase<M> {
    type Output = f64;
    type SampledOutput = SampledValues;

    fn evaluate(&self, solution: &State) -> Result<(Self::Output, BTreeSet<u64>)> {
        todo!()
    }

    fn partial_evaluate(&mut self, state: &State) -> Result<BTreeSet<u64>> {
        todo!()
    }

    fn required_ids(&self) -> BTreeSet<u64> {
        self.terms
            .keys()
            .flat_map(|monomial| monomial.ids())
            .map(|id| id.into_inner())
            .collect()
    }

    fn evaluate_samples(&self, samples: &Samples) -> Result<(Self::SampledOutput, BTreeSet<u64>)> {
        todo!()
    }
}
