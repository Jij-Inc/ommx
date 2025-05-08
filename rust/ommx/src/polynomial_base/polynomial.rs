use super::*;
use crate::{random::*, Monomial, VariableID};
use anyhow::{bail, Result};
use itertools::Itertools;
use maplit::hashset;
use proptest::prelude::*;
use std::collections::HashSet;
use std::ops::*;

pub type Polynomial = PolynomialBase<MonomialDyn>;

impl From<Linear> for Polynomial {
    fn from(l: Linear) -> Self {
        Self {
            terms: l.terms.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        }
    }
}

impl From<Quadratic> for Polynomial {
    fn from(q: Quadratic) -> Self {
        Self {
            terms: q.terms.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        }
    }
}

/// A sorted list of decision variable and parameter IDs
///
/// Note that this can store duplicated IDs. For example, `x1^2 * x2^3` is represented as `[1, 1, 2, 2, 2]`.
/// This is better than `[(1, 2), (2, 3)]` or `{1: 2, 2: 3}` style for low-degree polynomials.
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub struct MonomialDyn(Vec<u64>);

impl From<LinearMonomial> for MonomialDyn {
    fn from(m: LinearMonomial) -> Self {
        match m {
            LinearMonomial::Variable(id) => Self(vec![id.into_inner()]),
            LinearMonomial::Constant => Self::empty(),
        }
    }
}

impl From<QuadraticMonomial> for MonomialDyn {
    fn from(m: QuadraticMonomial) -> Self {
        match m {
            QuadraticMonomial::Pair(pair) => {
                Self(vec![pair.lower().into_inner(), pair.upper().into_inner()])
            }
            QuadraticMonomial::Linear(id) => Self(vec![id.into_inner()]),
            QuadraticMonomial::Constant => Self::empty(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Cannot convert {degree}-degree monomial to {max_degree}-degree")]
pub struct InvalidDegreeError {
    pub degree: Degree,
    pub max_degree: Degree,
}

impl TryFrom<&MonomialDyn> for LinearMonomial {
    type Error = InvalidDegreeError;
    fn try_from(m: &MonomialDyn) -> std::result::Result<Self, InvalidDegreeError> {
        match *m.degree() {
            1 => Ok(LinearMonomial::Variable(m.0[0].into())),
            0 => Ok(LinearMonomial::Constant),
            _ => Err(InvalidDegreeError {
                degree: m.degree(),
                max_degree: LinearMonomial::max_degree(),
            }),
        }
    }
}

impl TryFrom<&MonomialDyn> for QuadraticMonomial {
    type Error = InvalidDegreeError;
    fn try_from(m: &MonomialDyn) -> std::result::Result<Self, InvalidDegreeError> {
        match *m.degree() {
            2 => Ok(QuadraticMonomial::new_pair(m.0[0].into(), m.0[1].into())),
            1 => Ok(QuadraticMonomial::Linear(m.0[0].into())),
            0 => Ok(QuadraticMonomial::Constant),
            _ => Err(InvalidDegreeError {
                degree: m.degree(),
                max_degree: QuadraticMonomial::max_degree(),
            }),
        }
    }
}

impl From<Vec<u64>> for MonomialDyn {
    fn from(ids: Vec<u64>) -> Self {
        Self::new(ids)
    }
}

impl Deref for MonomialDyn {
    type Target = [u64];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Graded lexicographic order
///
/// - Higher grade comes first
/// - If grades are equal, lexicographic order is used
///
impl Ord for MonomialDyn {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let a = &self.0;
        let b = &other.0;
        if a.len() != b.len() {
            b.len().cmp(&a.len())
        } else {
            a.cmp(b)
        }
    }
}

impl PartialOrd for MonomialDyn {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl FromIterator<u64> for MonomialDyn {
    fn from_iter<I: IntoIterator<Item = u64>>(iter: I) -> Self {
        let ids = iter.into_iter().collect::<Vec<_>>();
        Self::new(ids)
    }
}

impl From<Option<u64>> for MonomialDyn {
    fn from(id: Option<u64>) -> Self {
        id.into_iter().collect()
    }
}

impl MonomialDyn {
    pub fn new(ids: Vec<u64>) -> Self {
        let mut ids = ids;
        ids.sort_unstable();
        Self(ids)
    }

    pub fn into_inner(self) -> Vec<u64> {
        self.0
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn iter(&self) -> impl Iterator<Item = &u64> {
        self.0.iter()
    }

    pub fn chunks(&self) -> Vec<(u64, usize)> {
        self.iter()
            .chunk_by(|&x| x)
            .into_iter()
            .map(|(key, group)| (*key, group.count()))
            .collect::<Vec<_>>()
    }
}

impl Mul for MonomialDyn {
    type Output = Self;

    fn mul(self, other: Self) -> Self::Output {
        let mut ids = self.0;
        ids.extend(other.0);
        ids.sort_unstable();
        Self(ids)
    }
}

impl Mul<LinearMonomial> for MonomialDyn {
    type Output = Self;
    fn mul(self, other: LinearMonomial) -> Self::Output {
        match other {
            LinearMonomial::Variable(id) => {
                let mut ids = self.0;
                ids.push(id.into_inner());
                ids.sort_unstable();
                Self(ids)
            }
            LinearMonomial::Constant => self,
        }
    }
}

impl Mul<QuadraticMonomial> for MonomialDyn {
    type Output = Self;
    fn mul(self, other: QuadraticMonomial) -> Self::Output {
        match other {
            QuadraticMonomial::Pair(pair) => {
                let mut ids = self.0;
                ids.push(pair.lower().into_inner());
                ids.push(pair.upper().into_inner());
                ids.sort_unstable();
                Self(ids)
            }
            QuadraticMonomial::Linear(id) => self * LinearMonomial::Variable(id),
            QuadraticMonomial::Constant => self,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, getset::CopyGetters)]
pub struct PolynomialParameters {
    #[getset(get_copy = "pub")]
    num_terms: usize,
    #[getset(get_copy = "pub")]
    max_degree: Degree,
    #[getset(get_copy = "pub")]
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

impl From<LinearParameters> for PolynomialParameters {
    fn from(p: LinearParameters) -> Self {
        PolynomialParameters {
            num_terms: p.num_terms(),
            max_degree: 1.into(),
            max_id: p.max_id(),
        }
    }
}

impl From<QuadraticParameters> for PolynomialParameters {
    fn from(p: QuadraticParameters) -> Self {
        PolynomialParameters {
            num_terms: p.num_terms(),
            max_degree: 2.into(),
            max_id: p.max_id(),
        }
    }
}

impl Monomial for MonomialDyn {
    type Parameters = PolynomialParameters;

    fn degree(&self) -> Degree {
        (self.0.len() as u32).into()
    }

    fn max_degree() -> Degree {
        u32::MAX.into()
    }

    fn ids(&self) -> Box<dyn Iterator<Item = VariableID> + '_> {
        Box::new(self.0.iter().map(|&id| VariableID::from(id)))
    }

    fn from_ids(ids: impl Iterator<Item = VariableID>) -> Option<Self> {
        Some(Self(ids.map(|id| id.into_inner()).collect()))
    }

    fn partial_evaluate(mut self, state: &State) -> (Self, f64) {
        let mut i = 0;
        let mut out = 1.0;
        while i < self.0.len() {
            let id = self.0[i];
            if let Some(value) = state.entries.get(&id) {
                // This keeps the order of the IDs
                // Since this `Vec` is usually small, we can use `remove` instead of `swap_remove`
                self.0.remove(i);
                out *= *value;
                continue;
            }
            i += 1;
        }
        (self, out)
    }

    fn arbitrary_uniques(p: Self::Parameters) -> BoxedStrategy<HashSet<Self>> {
        if p.max_degree == 0 {
            match p.num_terms {
                0 => return Just(HashSet::new()).boxed(),
                1 => return Just(hashset! { MonomialDyn::default() }).boxed(),
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
                let sub = MonomialDyn::arbitrary_uniques(sub_parameters);
                (ids, sub).prop_map(|(ids, mut sub)| {
                    sub.extend(ids);
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
                    MonomialDyn::arbitrary_uniques(p)
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
