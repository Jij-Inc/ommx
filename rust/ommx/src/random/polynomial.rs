use crate::v1::Polynomial;
use proptest::prelude::*;

use super::{arbitrary_coefficient_nonzero, multi_choose, unique_sorted_ids};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PolynomialParameters {
    pub num_terms: usize,
    pub max_degree: u32,
    pub max_id: u64,
}

impl PolynomialParameters {
    fn possible_max_terms(&self) -> usize {
        (0..=self.max_degree)
            .map(|d| multi_choose(self.max_id + 1, d as usize) as usize)
            .sum()
    }

    pub fn smaller(&self) -> impl Strategy<Value = Self> {
        (0..=self.max_id, 0..=self.max_degree, Just(self.num_terms)).prop_flat_map(
            move |(max_id, max_degree, num_terms)| {
                let small = Self {
                    max_id,
                    max_degree,
                    num_terms: 0,
                };
                (0..=std::cmp::min(num_terms, small.possible_max_terms())).prop_map(
                    move |num_terms| Self {
                        max_id,
                        num_terms,
                        max_degree,
                    },
                )
            },
        )
    }
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
        let max_num_terms = (0..=max_degree)
            .map(|d| multi_choose(max_id + 1, d as usize) as usize)
            .sum();
        assert!(
            num_terms <= max_num_terms,
            "num_terms({num_terms}) must be less than or equal to the possible maximum number of terms({max_num_terms})"
        );
        let ids = unique_sorted_ids(max_id, max_degree as usize, num_terms);
        let coefficients = proptest::collection::vec(arbitrary_coefficient_nonzero(), num_terms);
        (ids, coefficients)
            .prop_map(|(ids, coefficients)| ids.into_iter().zip(coefficients).collect())
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
        fn test_arbitrary_polynomial(p in Polynomial::arbitrary_with(PolynomialParameters { num_terms: 5, max_degree: 3, max_id: 10 })) {
            let mut count = 0;
            for (ids, _) in p.into_iter() {
                prop_assert!(ids.len() <= 3);
                for &id in ids.iter() {
                    prop_assert!(id <= 10);
                }
                count += 1;
            }
            prop_assert_eq!(count, 5);
        }
    }
}
