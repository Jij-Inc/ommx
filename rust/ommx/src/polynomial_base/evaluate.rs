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
        if state.entries.is_empty() {
            return Ok(BTreeSet::new());
        }
        let mut used = BTreeSet::new();
        let current = std::mem::take(&mut self.terms);
        for (monomial, coefficient) in current {
            let (new_monomial, value, ids) = monomial.partial_evaluate(state);
            used.extend(ids);
            match TryInto::<Coefficient>::try_into(value) {
                Ok(value) => {
                    self.terms.insert(new_monomial, value * coefficient);
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
        Ok(used)
    }

    fn required_ids(&self) -> BTreeSet<u64> {
        self.terms
            .keys()
            .flat_map(|monomial| monomial.ids())
            .map(|id| id.into_inner())
            .collect()
    }

    fn evaluate_samples(&self, _samples: &Samples) -> Result<(Self::SampledOutput, BTreeSet<u64>)> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::arbitrary_state;
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
                    let v1 = l1.evaluate(&state).unwrap().0;
                    let v2 = l2.evaluate(&state).unwrap().0;
                    let v3 = (&l1 $op &l2).evaluate(&state).unwrap().0;
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
}
