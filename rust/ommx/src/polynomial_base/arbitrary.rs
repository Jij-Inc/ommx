use super::*;
use proptest::prelude::*;

impl<M: Monomial> Arbitrary for PolynomialBase<M> {
    type Parameters = M::Parameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        M::arbitrary_uniques(p)
            .prop_flat_map(|uniques| {
                let num_terms = uniques.len();
                let coefficients = proptest::collection::vec(Coefficient::arbitrary(), num_terms);
                (coefficients, Just(uniques)).prop_map(move |(coefficients, uniques)| {
                    PolynomialBase {
                        terms: uniques.into_iter().zip(coefficients).collect(),
                    }
                })
            })
            .boxed()
    }
}
