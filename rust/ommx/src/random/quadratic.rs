use super::arbitrary_coefficient_nonzero;
use crate::{
    random::{unique_integer_pairs, FunctionParameters},
    v1::{Linear, Quadratic},
};
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

impl QuadraticParameters {
    fn possible_max_quad_terms(&self) -> usize {
        ((self.max_id + 2) * (self.max_id + 1) / 2) as usize
    }

    fn possible_max_linear_terms(&self) -> usize {
        (self.max_id + 1) as usize
    }

    // Possible maximum of the number of terms
    //
    // Note that `Quadratic` is not a binary function, x1 * x1 and x1 are different terms.
    fn possible_max_terms(&self) -> usize {
        self.possible_max_quad_terms() + self.possible_max_linear_terms()
    }

    fn linear_terms_range(&self) -> std::ops::RangeInclusive<usize> {
        let max = std::cmp::min(self.num_terms, self.possible_max_linear_terms());
        let min = if self.num_terms >= self.possible_max_quad_terms() {
            self.num_terms - self.possible_max_quad_terms()
        } else {
            0
        };
        min..=max
    }

    pub fn smaller(&self) -> impl Strategy<Value = Self> {
        (0..=self.max_id, Just(self.num_terms)).prop_flat_map(move |(max_id, num_terms)| {
            let small = Self {
                max_id,
                num_terms: 0,
            };
            (0..=std::cmp::min(num_terms, small.possible_max_terms()))
                .prop_map(move |num_terms| Self { max_id, num_terms })
        })
    }
}

impl Arbitrary for Quadratic {
    type Parameters = QuadraticParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        assert!(
            p.num_terms <= p.possible_max_terms(),
            "num_terms ({num_terms}) must be less than or equal to possible maximum ({possible_max_terms}) determined from max_id ({max_id})",
            num_terms = p.num_terms,
            max_id = p.max_id,
            possible_max_terms = p.possible_max_terms()
        );
        p.linear_terms_range()
            .prop_flat_map(move |num_linear| {
                let num_quad = p.num_terms - num_linear;
                assert!(
                    num_quad <= p.possible_max_quad_terms(),
                    "num_quad ({num_quad}) must be less than or equal to max_quad_terms({max_quad_terms})",
                    max_quad_terms = p.possible_max_quad_terms(),
                );

                let pairs = unique_integer_pairs(p.max_id, num_quad);
                let values = proptest::collection::vec(arbitrary_coefficient_nonzero(), num_quad);
                let linear = Linear::arbitrary_with(FunctionParameters{
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

        // (10 + 1) * (10 + 2) / 2 + (10 + 1) = 66 + 11 = 77
        #[test]
        fn test_arbitrary_quadratic_full(q in Quadratic::arbitrary_with(QuadraticParameters { num_terms: 77, max_id: 10 })) {
            prop_assert_eq!(q.into_iter().count(), 77);
        }
    }
}
