use super::{arbitrary_coefficient_nonzero, num_terms_and_max_id, LinearParameters};
use crate::v1::{Linear, Quadratic};
use proptest::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuadraticParameters {
    pub num_terms: usize,
    pub max_id: u64,
}

impl Default for QuadraticParameters {
    fn default() -> Self {
        Self {
            num_terms: 5,
            max_id: 10,
        }
    }
}

impl Arbitrary for Quadratic {
    type Parameters = QuadraticParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(
        QuadraticParameters { num_terms, max_id }: Self::Parameters,
    ) -> Self::Strategy {
        (0..=num_terms)
            .prop_flat_map(move |num_quad| {
                let num_linear = num_terms - num_quad;
                let quad = proptest::collection::hash_map(
                    arbitrary_key(max_id),
                    arbitrary_coefficient_nonzero(),
                    num_quad,
                );
                let linear = Linear::arbitrary_with(LinearParameters {
                    num_terms: num_linear,
                    max_id,
                });
                (quad, linear).prop_map(|(quad, linear)| {
                    let mut quad: Quadratic = quad.into_iter().collect();
                    quad.linear = Some(linear);
                    quad
                })
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        let QuadraticParameters { num_terms, max_id } = Self::Parameters::default();
        num_terms_and_max_id(num_terms, max_id)
            .prop_flat_map(move |(num_terms, max_id)| {
                Self::arbitrary_with(QuadraticParameters { num_terms, max_id })
            })
            .boxed()
    }
}

/// Generates a pair of ID `(i, j)` where `i <= j <= max_id`.
fn arbitrary_key(max_id: u64) -> impl Strategy<Value = (u64, u64)> {
    (0..=max_id).prop_flat_map(move |id1| (id1..=max_id).prop_map(move |id2| (id1, id2)))
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_arbitrary_quadratic(q in Quadratic::arbitrary_with(QuadraticParameters { num_terms: 5, max_id: 10 })) {
            let mut count = 0;
            for (ids, _) in q.into_iter() {
                for &id in ids.iter() {
                    prop_assert!(id <= 10);
                }
                count += 1;
            }
            prop_assert_eq!(count, 5);
        }
    }
}
