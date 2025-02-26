use crate::{random::unique_integers, v1::Linear};
use num::Zero;
use proptest::prelude::*;

use super::{arbitrary_coefficient, arbitrary_coefficient_nonzero, num_terms_and_max_id};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinearParameters {
    /// Number of non-zero terms in the linear function including the constant term.
    ///
    /// e.g. `x1 + x2 + 1` is 3 terms.
    ///
    /// ```rust
    /// use ommx::{random::{LinearParameters, random_deterministic}, v1::Linear};
    /// let lin: Linear = random_deterministic(LinearParameters { num_terms: 5, max_id: 10 });
    /// assert_eq!(lin.into_iter().count(), 5);
    /// ```
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
        assert!(
            num_terms <= max_id as usize + 1,
            "num_terms({num_terms}) must be less than or equal to max_id({max_id}) + 1 to ensure unique ids"
        );
        if num_terms == 0 {
            return Just(Linear::zero()).boxed();
        }
        arbitrary_coefficient()
            .prop_flat_map(move |constant| {
                let num_linear = if constant.abs() > f64::EPSILON {
                    num_terms - 1
                } else {
                    num_terms
                };
                let ids = unique_integers(0, max_id, num_terms);
                let coefficients =
                    proptest::collection::vec(arbitrary_coefficient_nonzero(), num_linear);
                (ids, coefficients).prop_map(move |(ids, coefficients)| {
                    Linear::new(
                        coefficients.iter().zip(ids.iter()).map(|(&c, &id)| (id, c)),
                        constant,
                    )
                })
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        let LinearParameters { num_terms, max_id } = Self::Parameters::default();
        num_terms_and_max_id(num_terms, max_id)
            .prop_flat_map(move |(num_terms, max_id)| {
                Self::arbitrary_with(LinearParameters { num_terms, max_id })
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_arbitrary_linear(l in Linear::arbitrary_with(LinearParameters { num_terms: 5, max_id: 10 })) {
            let mut count = 0;
            for (ids, _) in l.into_iter() {
                for &id in ids.iter() {
                    prop_assert!(id <= 10);
                }
                count += 1;
            }
            prop_assert_eq!(count, 5);
        }

        #[test]
        fn test_max_num_terms(l in Linear::arbitrary_with(LinearParameters { num_terms: 11, max_id: 10 })) {
            prop_assert_eq!(l.into_iter().count(), 11);
        }
    }
}
