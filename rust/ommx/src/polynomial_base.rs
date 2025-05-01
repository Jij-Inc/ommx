mod add;
mod approx;
mod arbitrary;
mod binary_ids;
mod convert;
mod degree;
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

use crate::Coefficient;
use proptest::strategy::BoxedStrategy;
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
};

/// Monomial, without coefficient
///
/// - [`Default`] must return the 0-degree monomial for the constant term
pub trait Monomial: Debug + Clone + Hash + Eq + Default + 'static {
    type Parameters: Default;

    fn degree(&self) -> Degree;
    fn max_degree() -> Degree;

    /// Generate non duplicated monomials
    fn arbitrary_uniques(parameters: Self::Parameters) -> BoxedStrategy<HashSet<Self>>;
}

/// Base struct for [`Linear`] and other polynomials
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolynomialBase<M: Monomial> {
    terms: HashMap<M, Coefficient>,
}

impl<M: Monomial> Default for PolynomialBase<M> {
    fn default() -> Self {
        Self {
            terms: HashMap::new(),
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
