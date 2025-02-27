use super::{arbitrary_coefficient_nonzero, multi_choose};
use crate::v1::{Function, Linear, Polynomial, Quadratic};
use anyhow::{bail, Result};
use num::Zero;
use proptest::{prelude::*, strategy::Union};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionParameters {
    /// Number of non-zero terms in the linear function including the constant term.
    ///
    /// e.g. `x1 + x2 + 1` is 3 terms.
    ///
    /// ```rust
    /// use ommx::{random::{FunctionParameters, random_deterministic}, v1::Function};
    /// let f: Function = random_deterministic(FunctionParameters { num_terms: 5, max_degree: 3, max_id: 10 });
    /// assert_eq!(f.into_iter().count(), 5);
    /// ```
    pub num_terms: usize,
    pub max_degree: u32,
    pub max_id: u64,
}

impl FunctionParameters {
    /// Evaluate possible max terms based on the `max_degree` and `max_id`.
    pub fn possible_max_terms(&self) -> usize {
        (0..=self.max_degree)
            .map(|d| multi_choose(self.max_id + 1, d as usize) as usize)
            .sum()
    }

    pub fn possible_max_quad_terms(&self) -> usize {
        ((self.max_id + 2) * (self.max_id + 1) / 2) as usize
    }

    pub fn possible_max_linear_terms(&self) -> usize {
        (self.max_id + 1) as usize
    }

    pub fn can_be_linear(&self) -> bool {
        self.num_terms <= 1 + self.possible_max_linear_terms()
    }

    pub fn can_be_quadratic(&self) -> bool {
        self.num_terms <= 1 + self.possible_max_linear_terms() + self.possible_max_quad_terms()
    }

    /// Validate the `num_terms` can be realized with the given `max_degree` and `max_id`.
    pub fn validate(&self) -> Result<()> {
        if self.num_terms > self.possible_max_terms() {
            bail!(
                "num_terms({num_terms}) must be less than or equal to the possible maximum number of terms({max_num_terms})",
                num_terms = self.num_terms,
                max_num_terms = self.possible_max_terms()
            )
        }
        Ok(())
    }

    /// Possible range for the largest degree term.
    pub fn largest_degree_term_range(&self) -> std::ops::RangeInclusive<usize> {
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

    pub fn linear_terms_range(&self) -> std::ops::RangeInclusive<usize> {
        let max = std::cmp::min(self.num_terms, self.possible_max_linear_terms());
        let min = if self.num_terms >= self.possible_max_quad_terms() {
            self.num_terms - self.possible_max_quad_terms()
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

impl Default for FunctionParameters {
    fn default() -> Self {
        Self {
            num_terms: 5,
            max_degree: 3,
            max_id: 10,
        }
    }
}

impl Arbitrary for Function {
    type Parameters = FunctionParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        p.validate().unwrap();
        let mut strategies = Vec::new();
        if p.num_terms == 0 {
            strategies.push(Just(Function::zero()).boxed());
        }
        if p.num_terms == 1 {
            strategies.push(
                arbitrary_coefficient_nonzero()
                    .prop_map(|c| Function::from(c))
                    .boxed(),
            );
        }
        if p.can_be_linear() {
            strategies.push(Linear::arbitrary_with(p).prop_map(Function::from).boxed());
        }
        if p.can_be_quadratic() {
            strategies.push(
                Quadratic::arbitrary_with(p)
                    .prop_map(Function::from)
                    .boxed(),
            )
        }
        strategies.push(
            Polynomial::arbitrary_with(p)
                .prop_map(Function::from)
                .boxed(),
        );
        Union::new(strategies).boxed()
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
        fn test_arbitrary_function(f in Function::arbitrary_with(FunctionParameters { num_terms: 5, max_degree: 3, max_id: 10 })) {
            let mut count = 0;
            for (ids, _) in f.into_iter() {
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
