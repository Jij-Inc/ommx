use crate::v1::Polynomial;
use num::Zero;
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

    fn largest_degree_term_range(&self) -> std::ops::RangeInclusive<usize> {
        let sub_max_terms = (0..self.max_degree)
            .map(|d| multi_choose(self.max_id + 1, d as usize) as usize)
            .sum::<usize>();
        let largest_max_terms = multi_choose(self.max_id + 1, self.max_degree as usize) as usize;
        let max = std::cmp::min(self.num_terms, largest_max_terms);
        let min = if self.num_terms >= sub_max_terms {
            self.num_terms - sub_max_terms
        } else {
            0
        };
        min..=max
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

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        assert!(
            p.num_terms <= p.possible_max_terms(),
            "num_terms({num_terms}) must be less than or equal to the possible maximum number of terms({max_num_terms})",
            num_terms = p.num_terms,
            max_num_terms = p.possible_max_terms()
        );
        if p.max_degree == 0 {
            if p.num_terms == 0 {
                return Just(Polynomial::zero()).boxed();
            }
            return arbitrary_coefficient_nonzero()
                .prop_map(|c| Polynomial::from(c))
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
                let sub = Self::arbitrary_with(PolynomialParameters {
                    num_terms: num_sub,
                    max_degree: p.max_degree - 1,
                    max_id: p.max_id,
                });

                (ids, coefficients, sub)
                    .prop_map(|(ids, coefficients, sub)| {
                        ids.into_iter()
                            .zip(coefficients)
                            .chain(sub.into_iter())
                            .collect()
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
