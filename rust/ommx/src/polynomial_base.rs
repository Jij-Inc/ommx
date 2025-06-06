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
mod substitute;

pub use binary_ids::*;
pub use degree::*;
pub use linear::*;
pub use parse::*;
pub use polynomial::*;
pub use quadratic::*;

use crate::{coeff, v1::State, Coefficient, VariableID};
use anyhow::{Context, Result};
use fnv::{FnvHashMap, FnvHashSet};
use num::{
    integer::{gcd, lcm},
    One,
};
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

impl<M: Monomial> From<M> for PolynomialBase<M> {
    fn from(monomial: M) -> Self {
        let mut terms = FnvHashMap::default();
        terms.insert(monomial, Coefficient::one());
        Self { terms }
    }
}

impl<M: Monomial> PolynomialBase<M> {
    pub fn one() -> Self {
        let mut terms = FnvHashMap::default();
        terms.insert(M::default(), coeff!(1.0));
        Self { terms }
    }

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

    /// Get a minimal positive factor `a` which make all coefficients of `a * self` integer.
    ///
    /// This returns `Coefficient::one()` for zero polynomial. See also <https://en.wikipedia.org/wiki/Primitive_part_and_content>.
    pub fn content_factor(&self) -> Result<Coefficient> {
        let mut numer_gcd = 0;
        let mut denom_lcm: i64 = 1;
        for coefficient in self.terms.values() {
            let r = num::Rational64::approximate_float(coefficient.into_inner())
                .context("Cannot approximate coefficient in 64-bit rational")?;
            numer_gcd = gcd(numer_gcd, *r.numer());
            denom_lcm
                .checked_mul(*r.denom())
                .context("Overflow detected while evaluating minimal integer coefficient multiplier. This means it is hard to make the all coefficient integer")?;
            denom_lcm = lcm(denom_lcm, *r.denom());
        }

        if numer_gcd == 0 {
            Ok(Coefficient::one())
        } else {
            let result = (denom_lcm as f64 / numer_gcd as f64).abs();
            Coefficient::try_from(result).context("Content factor should be non-zero")
        }
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

    #[test]
    fn test_content_factor() {
        use crate::linear;
        use ::approx::assert_abs_diff_eq;

        // Test with simple rational coefficients
        // 1/2 * x1 + 1/3 * x2 => content factor should be 6
        let p = coeff!(0.5) * linear!(1) + Coefficient::try_from(1.0 / 3.0).unwrap() * linear!(2);

        let factor = p.content_factor().unwrap();
        assert_abs_diff_eq!(factor.into_inner(), 6.0);

        // Test with integer coefficients (should return 1)
        let p = coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2);

        let factor = p.content_factor().unwrap();
        assert_eq!(factor, Coefficient::one());

        // Test with zero polynomial (empty terms)
        let p: Linear = PolynomialBase::default();
        let factor = p.content_factor().unwrap();
        assert_eq!(factor, Coefficient::one());

        // Test with more complex rational coefficients
        // 2/3 * x1 + 2/5 * x2 => content factor should be 15/2
        let p = Coefficient::try_from(2.0 / 3.0).unwrap() * linear!(1)
            + Coefficient::try_from(2.0 / 5.0).unwrap() * linear!(2);

        let factor = p.content_factor().unwrap();
        assert_abs_diff_eq!(factor.into_inner(), 15.0 / 2.0);

        // Test with PI (irrational numbers)
        use std::f64::consts::PI;
        let p = Coefficient::try_from(PI).unwrap() * linear!(1)
            + Coefficient::try_from(2.0 * PI).unwrap() * linear!(2);

        let factor = p.content_factor().unwrap();
        // For PI and 2*PI, the content factor should be approximately 1/PI
        assert_abs_diff_eq!(factor.into_inner(), 1.0 / PI);
    }
}
