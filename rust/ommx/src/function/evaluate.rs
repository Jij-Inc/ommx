use super::*;
use crate::{Evaluate, Sampled, VariableIDSet};

impl Evaluate for Function {
    type Output = f64;
    type SampledOutput = Sampled<f64>;

    fn evaluate(
        &self,
        solution: &crate::v1::State,
        atol: crate::ATol,
    ) -> crate::Result<Self::Output> {
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
    ) -> crate::Result<()> {
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
        samples: &Sampled<crate::v1::State>,
        atol: crate::ATol,
    ) -> crate::Result<Self::SampledOutput> {
        match self {
            Function::Zero => Ok(Sampled::constants(samples.ids().into_iter(), 0.0)),
            Function::Constant(c) => Ok(Sampled::constants(
                samples.ids().into_iter(),
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
    use crate::random::*;
    use ::approx::AbsDiffEq;
    use proptest::prelude::*;

    fn function_and_samples() -> impl Strategy<Value = (Function, Sampled<crate::v1::State>)> {
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
            for (sample_id, state) in samples.iter() {
                let expected = f.evaluate(state, crate::ATol::default()).unwrap();
                let actual = *evaluated.get(*sample_id).unwrap();
                prop_assert!(
                    actual.abs_diff_eq(&expected, 1e-9),
                    "sample_id = {sample_id:?}, expected = {expected}, actual = {actual}"
                );
            }
        }
    }
}
