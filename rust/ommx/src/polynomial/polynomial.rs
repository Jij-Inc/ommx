use crate::{random::multi_choose, sorted_ids::SortedIds, Monomial, VariableID};
use anyhow::{bail, Result};
use derive_more::{Deref, From};
use proptest::prelude::*;
use std::{collections::HashSet, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
pub struct Degree(u32);

impl Degree {
    pub fn into_inner(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for Degree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub struct PolynomialParameters {
    num_terms: usize,
    max_degree: Degree,
    max_id: VariableID,
}

impl PolynomialParameters {
    pub fn new(num_terms: usize, max_degree: Degree, max_id: VariableID) -> Result<Self> {
        let test = Self {
            num_terms,
            max_degree,
            max_id,
        };
        if num_terms > test.largest_max_degree_terms() + test.largest_sub_degree_terms() {
            bail!("Cannot create {num_terms} terms in {max_degree}-order polynomial with `max_id={max_id}`");
        }
        Ok(test)
    }

    /// Possible largest number of terms in the max degree terms.
    ///
    /// For example, when `max_degree=1`, we can create only `max_id+1` linear terms.
    fn largest_max_degree_terms(&self) -> usize {
        multi_choose(self.max_id.into_inner() + 1, self.max_degree.0 as usize) as usize
    }

    /// Possible largest number of terms in the sub-degree terms.
    fn largest_sub_degree_terms(&self) -> usize {
        let max_id = self.max_id.into_inner();
        (0..self.max_degree.0)
            .map(|d| multi_choose(max_id + 1, d as usize) as usize)
            .sum::<usize>()
    }
}

impl Default for PolynomialParameters {
    fn default() -> Self {
        PolynomialParameters {
            num_terms: 5,
            max_degree: 3.into(),
            max_id: 10.into(),
        }
    }
}

impl Monomial for SortedIds {
    type Parameters = PolynomialParameters;
    fn arbitrary_uniques(p: Self::Parameters) -> BoxedStrategy<HashSet<Self>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn largest_terms() {
        let p = PolynomialParameters::new(1, 1.into(), 3.into()).unwrap();
        // linear term can be [0, 1, 2, 3]
        assert_eq!(p.largest_max_degree_terms(), 4);
        // sub-degree term is only constant
        assert_eq!(p.largest_sub_degree_terms(), 1);
        assert!(PolynomialParameters::new(5, 1.into(), 3.into()).is_ok());
        assert!(PolynomialParameters::new(6, 1.into(), 3.into()).is_err());

        let p = PolynomialParameters::new(1, 0.into(), 3.into()).unwrap();
        // max degree term is only constant
        assert_eq!(p.largest_max_degree_terms(), 1);
        // sub-degree term must be empty
        assert_eq!(p.largest_sub_degree_terms(), 0);
        assert!(PolynomialParameters::new(1, 0.into(), 3.into()).is_ok());
        assert!(PolynomialParameters::new(2, 0.into(), 3.into()).is_err());

        let p = PolynomialParameters::new(1, 2.into(), 2.into()).unwrap();
        // Allowed max degree (=2) term is [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 2)]
        assert_eq!(p.largest_max_degree_terms(), 6);
        // sub-degree term can be [(), (0), (1), (2)]
        assert_eq!(p.largest_sub_degree_terms(), 4);
        assert!(PolynomialParameters::new(10, 2.into(), 2.into()).is_ok());
        assert!(PolynomialParameters::new(11, 2.into(), 2.into()).is_err());
    }
}
