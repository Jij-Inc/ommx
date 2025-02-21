use crate::{random::arbitrary_coefficient, sorted_ids::SortedIds, v1::Polynomial};
use proptest::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PolynomialParameters {
    pub num_terms: usize,
    pub max_degree: u32,
    pub max_id: u64,
}

impl Default for PolynomialParameters {
    fn default() -> Self {
        Self {
            num_terms: 5,
            max_degree: 3,
            max_id: 10,
        }
    }
}

impl Arbitrary for Polynomial {
    type Parameters = PolynomialParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(
        PolynomialParameters {
            num_terms,
            max_degree,
            max_id,
        }: Self::Parameters,
    ) -> Self::Strategy {
        let terms = proptest::collection::vec(
            (
                SortedIds::arbitrary_with((max_degree, max_id)),
                arbitrary_coefficient(),
            ),
            num_terms,
        );
        terms.prop_map(|terms| terms.into_iter().collect()).boxed()
    }

    fn arbitrary() -> Self::Strategy {
        let PolynomialParameters {
            num_terms,
            max_degree,
            max_id,
        } = Self::Parameters::default();
        (0..=num_terms, 0..=max_degree, 0..=max_id)
            .prop_flat_map(|(num_terms, max_degree, max_id)| {
                Self::arbitrary_with(PolynomialParameters {
                    num_terms,
                    max_degree,
                    max_id,
                })
            })
            .boxed()
    }
}
