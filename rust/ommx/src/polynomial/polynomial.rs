use super::*;
use crate::{random::*, sorted_ids::SortedIds, Monomial, VariableID};
use anyhow::{bail, Result};
use maplit::hashset;
use proptest::prelude::*;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        multi_choose(
            self.max_id.into_inner() + 1,
            self.max_degree.into_inner() as usize,
        ) as usize
    }

    /// Possible largest number of terms in the sub-degree terms.
    fn largest_sub_degree_terms(&self) -> usize {
        let max_id = self.max_id.into_inner();
        (0..self.max_degree.into_inner())
            .map(|d| multi_choose(max_id + 1, d as usize) as usize)
            .sum::<usize>()
    }
}

impl Arbitrary for PolynomialParameters {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (0..=4_u32, 0..=10_u64)
            .prop_flat_map(move |(max_degree, max_id)| {
                let p = Self {
                    num_terms: 0,
                    max_degree: max_degree.into(),
                    max_id: max_id.into(),
                };
                let max_num_terms = p.largest_max_degree_terms() + p.largest_sub_degree_terms();
                (0..=max_num_terms).prop_map(move |num_terms| {
                    PolynomialParameters::new(num_terms, max_degree.into(), max_id.into()).unwrap()
                })
            })
            .boxed()
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
        if p.max_degree == 0 {
            match p.num_terms {
                0 => return Just(HashSet::new()).boxed(),
                1 => return Just(hashset! { SortedIds::default() }).boxed(),
                _ => {
                    panic!("Invalid parameters for 0-degree polynomial: {p:?}");
                }
            }
        }
        let min = if p.num_terms >= p.largest_sub_degree_terms() {
            p.num_terms - p.largest_sub_degree_terms()
        } else {
            0
        };
        let max = p.largest_max_degree_terms().min(p.num_terms);
        (min..=max)
            .prop_flat_map(move |num_largest| {
                let ids = unique_sorted_ids(
                    p.max_id.into_inner(),
                    p.max_degree.into_inner() as usize,
                    num_largest,
                );
                let sub_parameters = PolynomialParameters {
                    num_terms: p.num_terms - num_largest,
                    max_degree: p.max_degree - 1,
                    max_id: p.max_id,
                };
                let sub = SortedIds::arbitrary_uniques(sub_parameters);
                (ids, sub).prop_map(|(ids, mut sub)| {
                    sub.extend(ids.into_iter());
                    sub
                })
            })
            .boxed()
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

    proptest! {
        #[test]
        fn test_polynomial(
            (p, monomials) in PolynomialParameters::arbitrary()
                .prop_flat_map(|p| {
                    SortedIds::arbitrary_uniques(p)
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
