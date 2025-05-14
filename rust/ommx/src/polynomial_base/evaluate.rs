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

    fn evaluate(&self, state: &State) -> Result<Self::Output> {
        let mut result = 0.0;
        for (monomial, coefficient) in self.iter() {
            let mut out = 1.0;
            for id in monomial.ids() {
                out *= state
                    .entries
                    .get(&id.into_inner())
                    .ok_or_else(|| anyhow!("Missing entry for id: {}", id.into_inner()))?;
            }
            result += coefficient.into_inner() * out;
        }
        Ok(result)
    }

    fn partial_evaluate(&mut self, state: &State) -> Result<()> {
        if state.entries.is_empty() {
            return Ok(());
        }
        let current = std::mem::take(&mut self.terms);
        for (monomial, coefficient) in current {
            let (new_monomial, value) = monomial.partial_evaluate(state);
            match TryInto::<Coefficient>::try_into(value) {
                Ok(value) => {
                    self.add_term(new_monomial, value * coefficient);
                }
                Err(crate::CoefficientError::Zero) => {
                    continue;
                }
                Err(e) => {
                    return Err(anyhow!(
                        "Partial evaluation yields non-finite coefficient: {}",
                        e
                    ));
                }
            }
        }
        Ok(())
    }

    fn required_ids(&self) -> BTreeSet<u64> {
        self.terms
            .keys()
            .flat_map(|monomial| monomial.ids())
            .map(|id| id.into_inner())
            .collect()
    }

    fn evaluate_samples(&self, samples: &Samples) -> Result<Self::SampledOutput> {
        samples.map(|state| self.evaluate(state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::*;
    use ::approx::AbsDiffEq;
    use proptest::prelude::*;

    fn polynomial_and_state<M: Monomial>() -> impl Strategy<Value = (PolynomialBase<M>, State)> {
        PolynomialBase::arbitrary().prop_flat_map(|p| {
            let state = arbitrary_state(p.required_ids());
            (Just(p), state)
        })
    }

    proptest! {
        #[test]
        fn test_evaluate_linear((linear, state) in polynomial_and_state::<LinearMonomial>()) {
            linear.evaluate(&state).unwrap();
        }

        #[test]
        fn test_evaluate_quadratic((quadratic, state) in polynomial_and_state::<QuadraticMonomial>()) {
            quadratic.evaluate(&state).unwrap();
        }

        #[test]
        fn test_evaluate_polynomial((polynomial, state) in polynomial_and_state::<MonomialDyn>()) {
            polynomial.evaluate(&state).unwrap();
        }
    }

    fn two_polynomial_and_state<M: Monomial>(
    ) -> impl Strategy<Value = (PolynomialBase<M>, PolynomialBase<M>, State)> {
        (PolynomialBase::arbitrary(), PolynomialBase::arbitrary()).prop_flat_map(|(p1, p2)| {
            let ids = p1
                .required_ids()
                .union(&p2.required_ids())
                .cloned()
                .collect();
            let state = arbitrary_state(ids);
            (Just(p1), Just(p2), state)
        })
    }

    macro_rules! test_ops_evaluate {
        ($monomial:ty, $name:ident, $op:tt) => {
            proptest! {
                #[test]
                fn $name(
                    (l1, l2, state) in two_polynomial_and_state::<$monomial>()
                ) {
                    let v1 = l1.evaluate(&state).unwrap();
                    let v2 = l2.evaluate(&state).unwrap();
                    let v3 = (&l1 $op &l2).evaluate(&state).unwrap();
                    prop_assert!((v1 $op v2).abs_diff_eq(&v3, 1e-9));
                }
            }
        };
    }

    test_ops_evaluate!(LinearMonomial, test_add_evaluate_linear, +);
    test_ops_evaluate!(LinearMonomial, test_mul_evaluate_linear, *);
    test_ops_evaluate!(QuadraticMonomial, test_add_evaluate_quadratic, +);
    test_ops_evaluate!(QuadraticMonomial, test_mul_evaluate_quadratic, *);
    test_ops_evaluate!(MonomialDyn, test_add_evaluate_polynomial, +);
    test_ops_evaluate!(MonomialDyn, test_mul_evaluate_polynomial, *);

    fn split_state(state: State) -> BoxedStrategy<(State, State)> {
        let ids: Vec<(u64, f64)> = state.entries.into_iter().collect();
        let flips = proptest::collection::vec(bool::arbitrary(), ids.len());
        (Just(ids), flips)
            .prop_map(|(ids, flips)| {
                let mut a = State::default();
                let mut b = State::default();
                for (flip, (id, value)) in flips.into_iter().zip(ids.into_iter()) {
                    if flip {
                        a.entries.insert(id, value);
                    } else {
                        b.entries.insert(id, value);
                    }
                }
                (a, b)
            })
            .boxed()
    }

    fn polynomial_and_state_split<M: Monomial>(
    ) -> impl Strategy<Value = (PolynomialBase<M>, State, State, State)> {
        polynomial_and_state::<M>()
            .prop_flat_map(|(poly, state)| {
                split_state(state.clone())
                    .prop_map(move |(state1, state2)| (poly.clone(), state.clone(), state1, state2))
            })
            .boxed()
    }

    macro_rules! test_partial_evaluate {
        ($monomial:ty, $name:ident) => {
            proptest! {
                #[test]
                fn $name(
                    (mut poly, state, s1, s2) in polynomial_and_state_split::<$monomial>()
                ) {
                    let v = poly.evaluate(&state).unwrap();
                    let _ = poly.partial_evaluate(&s1).unwrap();
                    let w = poly.evaluate(&s2).unwrap();
                    prop_assert!(w.abs_diff_eq(&v, 1e-9), "poly = {poly:?}, w = {w}, v = {v}");
                }
            }
        };
    }

    test_partial_evaluate!(LinearMonomial, test_partial_evaluate_linear);
    test_partial_evaluate!(QuadraticMonomial, test_partial_evaluate_quadratic);
    test_partial_evaluate!(MonomialDyn, test_partial_evaluate_polynomial);

    fn polynomial_and_samples<M: Monomial>() -> impl Strategy<Value = (PolynomialBase<M>, Samples)>
    {
        PolynomialBase::arbitrary()
            .prop_flat_map(|poly| {
                let ids = poly.required_ids();
                let state = arbitrary_state(ids);
                let samples = arbitrary_samples(SamplesParameters::default(), state);
                (Just(poly), samples)
            })
            .boxed()
    }

    proptest! {
        #[test]
        fn test_evaluate_samples(
            (poly, samples) in polynomial_and_samples::<LinearMonomial>()
        ) {
            let evaluated = poly.evaluate_samples(&samples).unwrap();
            let evaluated_each: SampledValues = samples.iter().map(|(parameter_id, state)| {
                let value = poly.evaluate(state).unwrap();
                (*parameter_id, value)
            }).collect();
            prop_assert!(evaluated.abs_diff_eq(&evaluated_each, 1e-9), "evaluated = {evaluated:?}, evaluated_each = {evaluated_each:?}");
        }
    }
}
