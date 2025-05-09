mod add;
mod approx;
mod arbitrary;
mod binary_ids;
mod convert;
mod degree;
mod evaluate;
mod linear;
mod mul;
mod parse;
mod polynomial;
mod quadratic;

pub use binary_ids::*;
pub use degree::*;
pub use linear::*;
pub use parse::*;
pub use polynomial::*;
pub use quadratic::*;

use crate::{v1::State, Coefficient, VariableID};
use fnv::{FnvHashMap, FnvHashSet};
use proptest::strategy::BoxedStrategy;
use std::{fmt::Debug, hash::Hash};

/// Monomial, without coefficient
///
/// - [`Default`] must return the 0-degree monomial for the constant term
pub trait Monomial: Debug + Clone + Hash + Eq + Default + 'static {
    type Parameters: Default;

    fn degree(&self) -> Degree;
    fn max_degree() -> Degree;

    fn ids(&self) -> Box<dyn Iterator<Item = VariableID> + '_>;
    /// Create a new monomial from a set of ids. If the size of IDs are too large, it will return `None`.
    fn from_ids(ids: impl Iterator<Item = VariableID>) -> Option<Self>;

    fn partial_evaluate(self, state: &State) -> (Self, f64);

    /// Generate non duplicated monomials
    fn arbitrary_uniques(parameters: Self::Parameters) -> BoxedStrategy<FnvHashSet<Self>>;
}

/// Base struct for [`Linear`] and other polynomials
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolynomialBase<M: Monomial> {
    terms: FnvHashMap<M, Coefficient>,
}

impl<M: Monomial> Default for PolynomialBase<M> {
    fn default() -> Self {
        Self {
            terms: Default::default(),
        }
    }
}

impl<M: Monomial> PolynomialBase<M> {
    pub fn add_term(&mut self, term: M, coefficient: Coefficient) {
        use std::collections::hash_map::Entry;
        match self.terms.entry(term) {
            Entry::Vacant(e) => {
                e.insert(coefficient);
            }
            Entry::Occupied(mut e) => {
                if let Some(added) = *e.get() + coefficient {
                    e.insert(added);
                } else {
                    e.remove();
                }
            }
        }
    }

    pub fn num_terms(&self) -> usize {
        self.terms.len()
    }

    pub fn degree(&self) -> Degree {
        let max_degree = M::max_degree();
        let mut current: Degree = 0.into();
        for term in self.terms.keys() {
            current = current.max(term.degree());
            // can be saturated
            if current == max_degree {
                break;
            }
        }
        current
    }

    pub fn contains(&self, term: &M) -> bool {
        self.terms.contains_key(term)
    }

    pub fn get(&self, term: &M) -> Option<Coefficient> {
        self.terms.get(term).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&M, &Coefficient)> {
        self.terms.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&M, &mut Coefficient)> {
        self.terms.iter_mut()
    }

    pub fn values(&self) -> impl Iterator<Item = &Coefficient> {
        self.terms.values()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Coefficient> {
        self.terms.values_mut()
    }

    pub fn keys(&self) -> impl Iterator<Item = &M> {
        self.terms.keys()
    }

    /// The maximum absolute value of the coefficients including the constant.
    ///
    /// `None` means this polynomial is exactly zero.
    pub fn max_coefficient_abs(&self) -> Option<Coefficient> {
        self.terms
            .values()
            .map(|coefficient| coefficient.abs())
            .max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::random_deterministic;

    /// The iteration order must be deterministic
    #[test]
    fn test_deterministic() {
        let p: Linear = random_deterministic(LinearParameters::new(3, 10.into()).unwrap());
        insta::assert_debug_snapshot!(p.iter().collect::<Vec<_>>(), @r###"
        [
            (
                Variable(
                    VariableID(
                        8,
                    ),
                ),
                Coefficient(
                    -4.973622349033379,
                ),
            ),
            (
                Variable(
                    VariableID(
                        7,
                    ),
                ),
                Coefficient(
                    -1.0,
                ),
            ),
            (
                Variable(
                    VariableID(
                        10,
                    ),
                ),
                Coefficient(
                    1.0,
                ),
            ),
        ]
        "###);
    }
}
