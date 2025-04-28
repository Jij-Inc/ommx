use crate::{
    random::{multi_choose, unique_integer_pairs},
    Monomial, VariableID,
};
use anyhow::{bail, Result};
use proptest::prelude::*;
use std::collections::HashSet;

use super::{LinearMonomial, LinearParameters};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuadraticMonomial {
    Pair(VariableIDPair),
    Linear(VariableID),
    #[default]
    Constant,
}

impl QuadraticMonomial {
    pub fn new_pair(a: VariableID, b: VariableID) -> Self {
        Self::Pair(VariableIDPair::new(a, b))
    }

    pub fn iter(&self) -> Box<dyn Iterator<Item = VariableID>> {
        match self {
            Self::Pair(pair) => Box::new(pair.iter()),
            Self::Linear(id) => Box::new(std::iter::once(*id)),
            Self::Constant => Box::new(std::iter::empty()),
        }
    }
}

impl From<LinearMonomial> for QuadraticMonomial {
    fn from(m: LinearMonomial) -> Self {
        match m {
            LinearMonomial::Variable(id) => Self::Linear(id),
            LinearMonomial::Constant => Self::Constant,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariableIDPair {
    lower: VariableID,
    upper: VariableID,
}

impl VariableIDPair {
    pub fn new(a: VariableID, b: VariableID) -> Self {
        if a <= b {
            Self { lower: a, upper: b }
        } else {
            Self { lower: b, upper: a }
        }
    }

    pub fn lower(&self) -> VariableID {
        self.lower
    }

    pub fn upper(&self) -> VariableID {
        self.upper
    }

    pub fn iter(&self) -> impl Iterator<Item = VariableID> {
        std::iter::once(self.lower).chain(std::iter::once(self.upper))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuadraticParameters {
    num_terms: usize,
    /// This ID is allowed. So when the `max_id=2`, `[0, 1, 2]` are allowed.
    max_id: VariableID,
}

impl QuadraticParameters {
    pub fn new(num_terms: usize, max_id: VariableID) -> Result<Self> {
        let test = Self { num_terms, max_id };
        if num_terms > test.largest_max_degree_terms() + test.largest_sub_degree_terms() {
            bail!("Cannot create {num_terms} terms in quadratic polynomial with `max_id={max_id}`");
        }
        Ok(test)
    }

    fn largest_max_degree_terms(&self) -> usize {
        multi_choose(self.max_id.into_inner() + 1, 2) as usize
    }

    fn largest_sub_degree_terms(&self) -> usize {
        let linear = self.max_id.into_inner() as usize + 1;
        linear + 1 /* constant */
    }
}

impl Arbitrary for QuadraticParameters {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (0..=10_u64)
            .prop_flat_map(move |max_id| {
                let p = Self {
                    num_terms: 0,
                    max_id: max_id.into(),
                };
                let max_num_terms = p.largest_max_degree_terms() + p.largest_sub_degree_terms();
                (0..=max_num_terms)
                    .prop_map(move |num_terms| Self::new(num_terms, max_id.into()).unwrap())
            })
            .boxed()
    }
}

impl Default for QuadraticParameters {
    fn default() -> Self {
        Self {
            num_terms: 5,
            max_id: 10.into(),
        }
    }
}

impl Monomial for QuadraticMonomial {
    type Parameters = QuadraticParameters;
    fn arbitrary_uniques(p: Self::Parameters) -> BoxedStrategy<HashSet<Self>> {
        let min = if p.num_terms >= p.largest_sub_degree_terms() {
            p.num_terms - p.largest_sub_degree_terms()
        } else {
            0
        };
        let max = p.largest_max_degree_terms().min(p.num_terms);
        (min..=max)
            .prop_flat_map(move |num_quad| {
                let ids = unique_integer_pairs(p.max_id.into_inner(), num_quad);
                let linear = LinearMonomial::arbitrary_uniques(
                    LinearParameters::new(p.num_terms - num_quad, p.max_id).unwrap(),
                );
                (ids, linear).prop_map(|(ids, sub)| {
                    sub.into_iter()
                        .map(|id| id.into())
                        .chain(
                            ids.into_iter()
                                .map(|(a, b)| QuadraticMonomial::new_pair(a.into(), b.into())),
                        )
                        .collect()
                })
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_quadratic(
            (p, monomials) in QuadraticParameters::arbitrary()
                .prop_flat_map(|p| {
                    QuadraticMonomial::arbitrary_uniques(p)
                        .prop_map(move |monomials| (p, monomials))
                }),
        ) {
            prop_assert_eq!(monomials.len(), p.num_terms);
            for monomial in monomials {
                for id in monomial.iter() {
                    prop_assert!(*id <= p.max_id.into_inner());
                }
            }
        }
    }
}
