use crate::v1::Linear;
use proptest::prelude::*;

use super::arbitrary_coefficient;

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
        let terms = proptest::collection::vec((0..=max_id, arbitrary_coefficient()), num_terms);
        let constant = arbitrary_coefficient();
        (terms, constant)
            .prop_map(|(terms, constant)| Linear::new(terms.into_iter(), constant))
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        let LinearParameters { num_terms, max_id } = Self::Parameters::default();
        (0..=num_terms, 0..=max_id)
            .prop_flat_map(|(num_terms, max_id)| {
                Self::arbitrary_with(LinearParameters { num_terms, max_id })
            })
            .boxed()
    }
}
