use super::*;
use crate::{v1::SampledValues, Evaluate, VariableIDSet};

impl Evaluate for Function {
    type Output = f64;
    type SampledOutput = crate::v1::SampledValues;

    fn evaluate(
        &self,
        solution: &crate::v1::State,
        atol: crate::ATol,
    ) -> anyhow::Result<Self::Output> {
        match self {
            Function::Zero => Ok(0.0),
            Function::Constant(c) => Ok(c.into_inner()),
            Function::Linear(f) => f.evaluate(solution, atol),
            Function::Quadratic(f) => f.evaluate(solution, atol),
            Function::Polynomial(f) => f.evaluate(solution, atol),
        }
    }

    fn partial_evaluate(
        &mut self,
        state: &crate::v1::State,
        atol: crate::ATol,
    ) -> anyhow::Result<()> {
        match self {
            Function::Linear(f) => f.partial_evaluate(state, atol),
            Function::Quadratic(f) => f.partial_evaluate(state, atol),
            Function::Polynomial(f) => f.partial_evaluate(state, atol),
            _ => Ok(()),
        }
    }

    fn required_ids(&self) -> VariableIDSet {
        match self {
            Function::Linear(f) => f.required_ids(),
            Function::Quadratic(f) => f.required_ids(),
            Function::Polynomial(f) => f.required_ids(),
            _ => VariableIDSet::default(),
        }
    }

    fn evaluate_samples(
        &self,
        samples: &crate::v1::Samples,
        atol: crate::ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        match self {
            Function::Zero => Ok(SampledValues::zeros(samples.ids().cloned())),
            Function::Constant(c) => Ok(SampledValues::constants(
                samples.ids().cloned(),
                c.into_inner(),
            )),
            Function::Linear(f) => f.evaluate_samples(samples, atol),
            Function::Quadratic(f) => f.evaluate_samples(samples, atol),
            Function::Polynomial(f) => f.evaluate_samples(samples, atol),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{random::*, v1::Samples};
    use ::approx::AbsDiffEq;
    use proptest::prelude::*;

    fn function_and_samples() -> impl Strategy<Value = (Function, Samples)> {
        Function::arbitrary()
            .prop_flat_map(|f| {
                let ids = f.required_ids();
                let state = arbitrary_state(ids);
                let samples = arbitrary_samples(SamplesParameters::default(), state);
                (Just(f), samples)
            })
            .boxed()
    }

    proptest! {
        #[test]
        fn test_evaluate_samples((f, samples) in function_and_samples()) {
            let evaluated = f.evaluate_samples(&samples, crate::ATol::default()).unwrap();
            let evaluated_each: SampledValues = samples.iter().map(|(parameter_id, state)| {
                let value = f.evaluate(state, crate::ATol::default()).unwrap();
                (*parameter_id, value)
            }).collect();
            prop_assert!(evaluated.abs_diff_eq(&evaluated_each, crate::ATol::default()), "evaluated = {evaluated:?}, evaluated_each = {evaluated_each:?}");
        }
    }
}
