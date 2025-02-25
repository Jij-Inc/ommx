use crate::v1::Linear;
use proptest::prelude::*;

use super::{arbitrary_coefficient, arbitrary_coefficient_nonzero};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinearParameters {
    pub num_terms: usize,
    pub max_id: u64,
}

impl Default for LinearParameters {
    fn default() -> Self {
        Self {
            num_terms: 5,
            max_id: 10,
        }
    }
}

impl Arbitrary for Linear {
    type Parameters = LinearParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(LinearParameters { num_terms, max_id }: Self::Parameters) -> Self::Strategy {
        // assert!(
        //     num_terms <= max_id as usize + 1,
        //     "num_terms({num_terms}) must be less than or equal to max_id({max_id}) + 1 to ensure unique ids"
        // );
        let ids = Just((0..=max_id).collect::<Vec<_>>()).prop_shuffle();
        let coefficients = proptest::collection::vec(arbitrary_coefficient_nonzero(), num_terms);
        let constant = arbitrary_coefficient();
        (ids, coefficients, constant)
            .prop_map(|(ids, coefficients, constant)| {
                Linear::new(
                    coefficients.iter().zip(ids.iter()).map(|(&c, &id)| (id, c)),
                    constant,
                )
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        let LinearParameters { num_terms, max_id } = Self::Parameters::default();
        // Only samples where `num_terms <= max_id + 1`
        (0..=max_id)
            .prop_flat_map(move |max_id| {
                let max_num_terms = std::cmp::min(max_id as usize + 1, num_terms);
                (0..=max_num_terms).prop_flat_map(move |num_terms| {
                    Self::arbitrary_with(LinearParameters { num_terms, max_id })
                })
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use crate::v1::linear::Term;

    use super::*;

    proptest! {
        #[test]
        fn test_arbitrary_linear(l in Linear::arbitrary_with(LinearParameters { num_terms: 5, max_id: 10 })) {
            prop_assert!(l.terms.len() == 5);
            for Term {id, coefficient} in l.terms {
                prop_assert!(id <= 10);
                prop_assert!(coefficient != 0.0);
            }
        }
    }
}
