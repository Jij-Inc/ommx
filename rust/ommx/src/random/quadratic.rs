use super::{arbitrary_coefficient, num_terms_and_max_id, LinearParameters};
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
        let terms = proptest::collection::vec(
            ((0..=max_id, 0..=max_id), arbitrary_coefficient()),
            num_terms,
        );
        let linear = Linear::arbitrary_with(LinearParameters { num_terms, max_id });
        (terms, linear)
            .prop_map(|(terms, linear)| {
                let mut quad: Quadratic = terms.into_iter().collect();
                quad.linear = Some(linear);
                quad
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
