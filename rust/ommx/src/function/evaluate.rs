use super::*;
use crate::Evaluate;

impl Evaluate for Function {
    type Output = f64;
    type SampledOutput = crate::v1::SampledValues;

    fn evaluate(&self, solution: &crate::v1::State) -> anyhow::Result<Self::Output> {
        match self {
            Function::Zero => Ok(0.0),
            Function::Constant(c) => Ok(c.into_inner()),
            Function::Linear(f) => f.evaluate(solution),
            Function::Quadratic(f) => f.evaluate(solution),
            Function::Polynomial(f) => f.evaluate(solution),
        }
    }

    fn partial_evaluate(&mut self, state: &crate::v1::State) -> anyhow::Result<()> {
        match self {
            Function::Linear(f) => f.partial_evaluate(state),
            Function::Quadratic(f) => f.partial_evaluate(state),
            Function::Polynomial(f) => f.partial_evaluate(state),
            _ => Ok(()),
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
    ) -> anyhow::Result<Self::SampledOutput> {
        match self {
            Function::Zero => todo!(),
            Function::Constant(_) => todo!(),
            Function::Linear(f) => f.evaluate_samples(samples),
            Function::Quadratic(f) => f.evaluate_samples(samples),
            Function::Polynomial(f) => f.evaluate_samples(samples),
        }
    }
}
