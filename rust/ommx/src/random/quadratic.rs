use super::arbitrary_coefficient_nonzero;
use crate::{
    random::{unique_integer_pairs, FunctionParameters},
    v1::{Linear, Quadratic},
};
use num::Zero;
use proptest::prelude::*;

impl Arbitrary for Quadratic {
    type Parameters = FunctionParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(mut p: Self::Parameters) -> Self::Strategy {
        p.validate().unwrap();
        assert!(
            p.can_be_quadratic(),
            "FunctionParameters ({p:?}) cannot be realized as a Quadratic",
        );
        if p.num_terms == 0 {
            return Just(Quadratic::zero()).boxed();
        }
        p.max_degree = 2;
        p.largest_degree_term_range()
            .prop_flat_map(move |num_quad| {
                let num_linear = p.num_terms - num_quad;
                let pairs = unique_integer_pairs(p.max_id, num_quad);
                let values = proptest::collection::vec(arbitrary_coefficient_nonzero(), num_quad);
                let linear = Linear::arbitrary_with(FunctionParameters {
                    num_terms: num_linear,
                    max_degree: 1,
                    max_id: p.max_id,
                });
                (pairs, values, linear).prop_map(|(pairs, values, linear)| {
                    let mut quad: Quadratic = pairs.into_iter().zip(values).collect();
                    quad.linear = Some(linear);
                    quad
                })
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
        fn test_arbitrary_quadratic(q in Quadratic::arbitrary_with(FunctionParameters { num_terms: 5, max_degree: 2, max_id: 10 })) {
            let mut count = 0;
            for (ids, _) in q.into_iter() {
                for &id in ids.iter() {
                    prop_assert!(id <= 10);
                }
                count += 1;
            }
            prop_assert_eq!(count, 5);
        }

        // (10 + 1) * (10 + 2) / 2 + (10 + 1) = 66 + 11 = 77
        #[test]
        fn test_arbitrary_quadratic_full(q in Quadratic::arbitrary_with(FunctionParameters { num_terms: 77, max_degree: 2, max_id: 10 })) {
            prop_assert_eq!(q.into_iter().count(), 77);
        }
    }
}
