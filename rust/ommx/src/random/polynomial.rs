use crate::v1::Polynomial;
use num::Zero;
use proptest::prelude::*;

use super::{arbitrary_coefficient_nonzero, unique_sorted_ids, FunctionParameters};

impl Arbitrary for Polynomial {
    type Parameters = FunctionParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        p.validate().unwrap();
        if p.max_degree == 0 {
            if p.num_terms == 0 {
                return Just(Polynomial::zero()).boxed();
            }
            return arbitrary_coefficient_nonzero()
                .prop_map(Polynomial::from)
                .boxed();
        }
        p.largest_degree_term_range()
            .prop_flat_map(move |num_largest| {
                // The largest degree terms
                let ids = unique_sorted_ids(p.max_id, p.max_degree as usize, num_largest);
                let coefficients =
                    proptest::collection::vec(arbitrary_coefficient_nonzero(), num_largest);

                // The remaining terms
                let num_sub = p.num_terms - num_largest;
                let sub = Self::arbitrary_with(FunctionParameters {
                    num_terms: num_sub,
                    max_degree: p.max_degree - 1,
                    max_id: p.max_id,
                });

                (ids, coefficients, sub)
                    .prop_map(|(ids, coefficients, sub)| {
                        ids.into_iter().zip(coefficients).chain(&sub).collect()
                    })
                    .boxed()
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        Self::Parameters::default()
            .smaller()
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_arbitrary_polynomial(p in Polynomial::arbitrary_with(FunctionParameters { num_terms: 5, max_degree: 3, max_id: 10 })) {
            let mut count = 0;
            for (ids, _) in p.into_iter() {
                prop_assert!(ids.len() <= 3);
                for &id in ids.iter() {
                    prop_assert!(id <= 10.into());
                }
                count += 1;
            }
            prop_assert_eq!(count, 5);
        }
    }
}
