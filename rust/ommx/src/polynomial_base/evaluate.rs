use super::*;
use crate::{
    v1::{SampledValues, Samples, State},
    Evaluate,
};
use anyhow::{anyhow, Result};
use std::collections::BTreeSet;

impl<M: Monomial> Evaluate for PolynomialBase<M> {
    type Output = f64;
    type SampledOutput = SampledValues;

    fn evaluate(&self, state: &State) -> Result<(Self::Output, BTreeSet<u64>)> {
        let mut result = 0.0;
        let mut ids = BTreeSet::new();
        for (monomial, coefficient) in self.iter() {
            let mut out = 1.0;
            for id in monomial.ids() {
                out *= state
                    .entries
                    .get(&id.into_inner())
                    .ok_or(anyhow!("Missing entry for id: {}", id.into_inner()))?;
            }
            result += coefficient.into_inner() * out;
            ids.extend(monomial.ids().map(|id| id.into_inner()));
        }
        Ok((result, ids))
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
