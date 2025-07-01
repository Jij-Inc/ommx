use super::*;
use crate::{random::*, Monomial, VariableID, VariableIDPair};
use anyhow::{bail, Result};
use itertools::Itertools;
use proptest::prelude::*;
use smallvec::{smallvec, SmallVec};
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
pub struct MonomialDyn(SmallVec<[VariableID; 3]>);

impl From<LinearMonomial> for MonomialDyn {
    fn from(m: LinearMonomial) -> Self {
        match m {
            LinearMonomial::Variable(id) => Self(smallvec![id]),
            LinearMonomial::Constant => Self::empty(),
        }
    }
}

impl From<QuadraticMonomial> for MonomialDyn {
    fn from(m: QuadraticMonomial) -> Self {
        match m {
            QuadraticMonomial::Pair(pair) => Self(smallvec![pair.lower(), pair.upper()]),
            QuadraticMonomial::Linear(id) => Self(smallvec![id]),
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
            1 => Ok(LinearMonomial::Variable(m.0[0])),
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
            2 => Ok(QuadraticMonomial::new_pair(m.0[0], m.0[1])),
            1 => Ok(QuadraticMonomial::Linear(m.0[0])),
            0 => Ok(QuadraticMonomial::Constant),
            _ => Err(InvalidDegreeError {
                degree: m.degree(),
                max_degree: QuadraticMonomial::max_degree(),
            }),
        }
    }
}

impl From<Vec<VariableID>> for MonomialDyn {
    fn from(ids: Vec<VariableID>) -> Self {
        Self::new(ids)
    }
}

impl Deref for MonomialDyn {
    type Target = [VariableID];
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

impl FromIterator<VariableID> for MonomialDyn {
    fn from_iter<I: IntoIterator<Item = VariableID>>(iter: I) -> Self {
        let ids = iter.into_iter().collect::<Vec<_>>();
        Self::new(ids)
    }
}

impl From<Option<VariableID>> for MonomialDyn {
    fn from(id: Option<VariableID>) -> Self {
        id.into_iter().collect()
    }
}

impl MonomialDyn {
    pub fn new(ids: Vec<VariableID>) -> Self {
        let mut ids = ids;
        ids.sort_unstable();
        Self(ids.into())
    }

    pub fn into_inner(self) -> SmallVec<[VariableID; 3]> {
        self.0
    }

    pub fn empty() -> Self {
        Self(SmallVec::new())
    }

    pub fn iter(&self) -> impl Iterator<Item = &VariableID> {
        self.0.iter()
    }

    pub fn chunks(&self) -> Vec<(VariableID, usize)> {
        self.iter()
            .chunk_by(|&x| x)
            .into_iter()
            .map(|(key, group)| (*key, group.count()))
            .collect::<Vec<_>>()
    }
}

impl IntoIterator for MonomialDyn {
    type Item = VariableID;
    type IntoIter = <SmallVec<[VariableID; 3]> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a MonomialDyn {
    type Item = &'a VariableID;
    type IntoIter = std::slice::Iter<'a, VariableID>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
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
                ids.push(id);
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
                ids.push(pair.lower());
                ids.push(pair.upper());
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

    fn as_linear(&self) -> Option<VariableID> {
        if self.0.len() == 1 {
            Some(self.0[0])
        } else {
            None
        }
    }

    fn as_quadratic(&self) -> Option<VariableIDPair> {
        if self.0.len() == 2 {
            Some(VariableIDPair::new(self.0[0], self.0[1]))
        } else {
            None
        }
    }

    fn reduce_binary_power(&mut self, binary_ids: &VariableIDSet) -> bool {
        if self.0.len() <= 1 {
            // No need to reduce if the degree is already linear or constant
            return false;
        }
        let mut current = self.0[0];
        let mut i = 1;
        let mut changed = false;
        while i < self.0.len() {
            if self.0[i] == current {
                // Found a duplicate ID, reduce it
                if binary_ids.contains(&current) {
                    // If the ID is in the binary IDs, we can reduce it
                    self.0.remove(i);
                    changed = true;
                    continue;
                }
            } else {
                current = self.0[i];
            }
            i += 1;
        }
        changed
    }

    fn ids(&self) -> Box<dyn Iterator<Item = VariableID> + '_> {
        Box::new(self.0.iter().copied())
    }

    fn from_ids(ids: impl Iterator<Item = VariableID>) -> Option<Self> {
        Some(Self(ids.collect()))
    }

    fn partial_evaluate(mut self, state: &State) -> (Self, f64) {
        let mut i = 0;
        let mut out = 1.0;
        while i < self.0.len() {
            let id = self.0[i];
            if let Some(value) = state.entries.get(&id.into_inner()) {
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

    fn arbitrary_uniques(p: Self::Parameters) -> BoxedStrategy<FnvHashSet<Self>> {
        if p.max_degree == 0 {
            match p.num_terms {
                0 => return Just(Default::default()).boxed(),
                1 => return Just([MonomialDyn::default()].into_iter().collect()).boxed(),
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
                    max_degree: (p.max_degree.into_inner() - 1).into(),
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
    fn test_reduce_binary_power() {
        // Test case 1: x1 * x1 * x2 should reduce to x1 * x2 when x1 is binary
        let x1 = VariableID::from(1);
        let x2 = VariableID::from(2);

        // Create monomial x1 * x1 * x2
        let mut monomial = MonomialDyn::new(vec![x1, x1, x2]);

        // Create binary variable set containing x1
        let mut binary_ids = VariableIDSet::default();
        binary_ids.insert(x1);

        // Apply reduction
        let changed = monomial.reduce_binary_power(&binary_ids);

        // Verify the result
        assert!(changed);
        assert_eq!(monomial.0.len(), 2);
        assert_eq!(monomial.0[0], x1);
        assert_eq!(monomial.0[1], x2);

        // Test case 2: No change when variables are not binary
        let x3 = VariableID::from(3);
        let x4 = VariableID::from(4);
        
        // Create monomial x3 * x3 * x4
        let mut monomial2 = MonomialDyn::new(vec![x3, x3, x4]);
        
        // Binary set doesn't contain x3
        let changed2 = monomial2.reduce_binary_power(&binary_ids);
        
        // Should not change
        assert!(!changed2);
        assert_eq!(monomial2.0.len(), 3);
        assert_eq!(monomial2.0[0], x3);
        assert_eq!(monomial2.0[1], x3);
        assert_eq!(monomial2.0[2], x4);

        // Test case 3: No change for linear monomial
        let mut monomial3 = MonomialDyn::new(vec![x1]);
        let changed3 = monomial3.reduce_binary_power(&binary_ids);
        
        // Should not change (already linear)
        assert!(!changed3);
        assert_eq!(monomial3.0.len(), 1);
        assert_eq!(monomial3.0[0], x1);

        // Test case 4: No change for constant monomial
        let mut monomial4 = MonomialDyn::new(vec![]);
        let changed4 = monomial4.reduce_binary_power(&binary_ids);
        
        // Should not change (constant)
        assert!(!changed4);
        assert_eq!(monomial4.0.len(), 0);

        // Test case 5: Multiple binary variables x1^3 * x2^2 -> x1 * x2 when both are binary
        let mut monomial5 = MonomialDyn::new(vec![x1, x1, x1, x2, x2]);
        
        // Add x2 to binary set
        let mut binary_ids2 = VariableIDSet::default();
        binary_ids2.insert(x1);
        binary_ids2.insert(x2);
        
        let changed5 = monomial5.reduce_binary_power(&binary_ids2);
        
        // Should reduce to x1 * x2
        assert!(changed5);
        assert_eq!(monomial5.0.len(), 2);
        assert_eq!(monomial5.0[0], x1);
        assert_eq!(monomial5.0[1], x2);
    }

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
                    prop_assert!(*id <= p.max_id);
                }
            }
        }
    }
}
