use super::*;
use crate::Evaluate;

impl Evaluate for Function {
    type Output = f64;
    type SampledOutput = crate::v1::SampledValues;

    fn evaluate(
        &self,
        solution: &crate::v1::State,
    ) -> anyhow::Result<(Self::Output, std::collections::BTreeSet<u64>)> {
        use std::collections::BTreeSet;
        match self {
            Function::Zero => Ok((0.0, BTreeSet::new())),
            Function::Constant(c) => Ok((c.into_inner(), BTreeSet::new())),
            Function::Linear(f) => f.evaluate(solution),
            Function::Quadratic(f) => f.evaluate(solution),
            Function::Polynomial(f) => f.evaluate(solution),
        }
    }

    fn partial_evaluate(
        &mut self,
        state: &crate::v1::State,
    ) -> anyhow::Result<std::collections::BTreeSet<u64>> {
        use std::collections::BTreeSet;
        match self {
            Function::Zero => Ok(BTreeSet::new()),
            Function::Constant(_) => Ok(BTreeSet::new()),
            Function::Linear(f) => f.partial_evaluate(state),
            Function::Quadratic(f) => f.partial_evaluate(state),
            Function::Polynomial(f) => f.partial_evaluate(state),
        }
    }

    fn required_ids(&self) -> std::collections::BTreeSet<u64> {
        match self {
            Function::Zero => std::collections::BTreeSet::new(),
            Function::Constant(_) => std::collections::BTreeSet::new(),
            Function::Linear(f) => f.required_ids(),
            Function::Quadratic(f) => f.required_ids(),
            Function::Polynomial(f) => f.required_ids(),
        }
    }

    fn evaluate_samples(
        &self,
        samples: &crate::v1::Samples,
    ) -> anyhow::Result<(Self::SampledOutput, std::collections::BTreeSet<u64>)> {
        match self {
            Function::Zero => todo!(),
            Function::Constant(_) => todo!(),
            Function::Linear(f) => f.evaluate_samples(samples),
            Function::Quadratic(f) => f.evaluate_samples(samples),
            Function::Polynomial(f) => f.evaluate_samples(samples),
        }
    }
}
