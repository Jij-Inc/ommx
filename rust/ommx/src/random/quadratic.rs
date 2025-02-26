use super::{arbitrary_coefficient_nonzero, LinearParameters};
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

                let quad = proptest::collection::hash_map(
                    arbitrary_key(p.max_id),
                    arbitrary_coefficient_nonzero(),
                    num_quad,
                );
                let linear = Linear::arbitrary_with(LinearParameters {
                    num_terms: num_linear,
                    max_id: p.max_id,
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
        Self::Parameters::default()
            .smaller()
            .prop_flat_map(Self::arbitrary_with)
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
