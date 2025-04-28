mod add;
mod approx;
mod arbitrary;
mod convert;
mod degree;
mod linear;
mod parse;
mod polynomial;

pub use degree::*;
pub use linear::*;

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

    pub fn contains(&self, term: &M) -> bool {
        self.terms.contains_key(term)
    }

    pub fn get(&self, term: &M) -> Option<Coefficient> {
        self.terms.get(term).copied()
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
