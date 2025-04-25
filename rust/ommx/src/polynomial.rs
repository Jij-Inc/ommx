mod add;
mod approx;
mod convert;

use crate::{Coefficient, VariableID};
use std::{collections::HashMap, hash::Hash};

/// Monomial
///
/// - [`Default`] must return the 0-degree monomial for the constant term
pub trait Monomial: Clone + Hash + Eq + Default {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Polynomial<M: Monomial> {
    terms: HashMap<M, Coefficient>,
}

impl<M: Monomial> Default for Polynomial<M> {
    fn default() -> Self {
        Self {
            terms: HashMap::new(),
        }
    }
}

impl<M: Monomial> Polynomial<M> {
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

/// Linear function only contains monomial of degree 1 or constant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LinearMonomial {
    Variable(VariableID),
    #[default]
    Constant,
}

impl Monomial for LinearMonomial {}
