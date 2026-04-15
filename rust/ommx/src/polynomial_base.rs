mod add;
mod approx;
mod arbitrary;
mod binary_ids;
mod convert;
mod degree;
mod evaluate;
mod linear;
mod logical_memory;
mod mul;
mod parse;
mod polynomial;
mod quadratic;
mod serialize;
mod substitute;

pub use binary_ids::*;
pub use degree::*;
pub use linear::*;
pub use parse::*;
pub use polynomial::*;
pub use quadratic::*;

use crate::{coeff, v1::State, Coefficient, VariableID, VariableIDSet};
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
pub trait Monomial: Into<MonomialDyn> + Debug + Clone + Hash + Eq + Default + 'static {
    type Parameters: Default;

    fn degree(&self) -> Degree;
    fn max_degree() -> Degree;

    fn as_linear(&self) -> Option<VariableID>;
    fn as_quadratic(&self) -> Option<VariableIDPair>;

    /// Reduce power to linear `x^n -> x` for binary variables.
    ///
    /// This returns `true` if the monomial is reduced.
    fn reduce_binary_power(&mut self, binary_ids: &VariableIDSet) -> bool;

    fn ids(&self) -> Box<dyn Iterator<Item = VariableID> + '_>;
    /// Create a new monomial from a set of ids. If the size of IDs are too large, it will return `None`.
    fn from_ids(ids: impl Iterator<Item = VariableID>) -> Option<Self>;

    fn partial_evaluate(self, state: &State) -> (Self, f64);

    /// Generate non duplicated monomials
    fn arbitrary_uniques(parameters: Self::Parameters) -> BoxedStrategy<FnvHashSet<Self>>;
}

/// Base struct for [`Linear`] and other polynomials
#[derive(Clone, PartialEq, Eq)]
pub struct PolynomialBase<M: Monomial> {
    terms: FnvHashMap<M, Coefficient>,
}

impl<M: Monomial + serde::Serialize> serde::Serialize for PolynomialBase<M> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.terms.len()))?;
        for (monomial, coefficient) in &self.terms {
            map.serialize_entry(monomial, coefficient)?;
        }
        map.end()
    }
}

impl<'de, M: Monomial + serde::Deserialize<'de>> serde::Deserialize<'de> for PolynomialBase<M> {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let terms = FnvHashMap::<M, Coefficient>::deserialize(deserializer)?;
        Ok(Self { terms })
    }
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

    /// Get constant term, zero if not present
    pub fn constant_term(&self) -> f64 {
        self.get(&M::default())
            .map(|c| c.into_inner())
            .unwrap_or(0.0)
    }

    /// Get terms of specific degree as an iterator
    pub fn terms_by_degree(&self, degree: Degree) -> impl Iterator<Item = (&M, &Coefficient)> {
        self.iter()
            .filter(move |(monomial, _)| monomial.degree() == degree)
    }

    /// Get linear terms as an iterator over (VariableID, Coefficient)
    pub fn linear_terms(&self) -> impl Iterator<Item = (VariableID, Coefficient)> + '_ {
        self.iter()
            .filter_map(|(monomial, coeff)| monomial.as_linear().map(|id| (id, *coeff)))
    }

    /// Get quadratic terms as an iterator over (VariableIDPair, Coefficient)
    pub fn quadratic_terms(&self) -> impl Iterator<Item = (VariableIDPair, Coefficient)> + '_ {
        self.iter()
            .filter_map(|(monomial, coeff)| monomial.as_quadratic().map(|pair| (pair, *coeff)))
    }

    pub fn reduce_binary_power(&mut self, binary_ids: &VariableIDSet) -> bool {
        let mut reduced = false;
        let mut new = Self::default();
        for (monomial, coefficient) in &self.terms {
            let mut m = monomial.clone();
            reduced |= m.reduce_binary_power(binary_ids);
            new.add_term(m, *coefficient);
        }
        *self = new;
        reduced
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
                    VariableID(8),
                ),
                Coefficient(-1),
            ),
            (
                Variable(
                    VariableID(7),
                ),
                Coefficient(-0.27550031881072173),
            ),
            (
                Variable(
                    VariableID(10),
                ),
                Coefficient(4.520657493715473),
            ),
        ]
        "###);
    }

    #[test]
    fn test_polynomial_base_reduce_binary_power() {
        use crate::{linear, quadratic};
        use ::approx::assert_abs_diff_eq;

        // Test case 1: Linear polynomial - no change expected
        let original_linear = coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2) + coeff!(1.0);
        let mut linear_poly = original_linear.clone();

        let mut binary_ids = crate::variable_ids!(1);

        let changed = linear_poly.reduce_binary_power(&binary_ids);
        assert!(!changed); // Linear terms should not change
        assert_abs_diff_eq!(linear_poly, original_linear);

        // Test case 2: Quadratic polynomial with binary variable
        // x1^2 + x1*x2 + x2^2 + 4 -> x1 + x1*x2 + x2^2 + 4 when x1 is binary
        let mut quad_poly = quadratic!(1, 1)
            + coeff!(2.0) * quadratic!(1, 2)
            + coeff!(3.0) * quadratic!(2, 2)
            + coeff!(4.0);

        let expected = quadratic!(1)
            + coeff!(2.0) * quadratic!(1, 2)
            + coeff!(3.0) * quadratic!(2, 2)
            + coeff!(4.0);

        let changed2 = quad_poly.reduce_binary_power(&binary_ids);
        assert!(changed2);
        assert_abs_diff_eq!(quad_poly, expected);

        // Test case 3: Multiple binary variables
        binary_ids.extend(crate::variable_ids!(2, 3));

        // x1^2 + x2^2 + x3^2 + x1*x2 -> x1 + x2 + x3 + x1*x2
        let mut quad_poly2 = coeff!(5.0) * quadratic!(1, 1)
            + coeff!(6.0) * quadratic!(2, 2)
            + coeff!(7.0) * quadratic!(3, 3)
            + coeff!(8.0) * quadratic!(1, 2);

        let expected2 = coeff!(5.0) * quadratic!(1)  // x1^2 -> x1
            + coeff!(6.0) * quadratic!(2)  // x2^2 -> x2
            + coeff!(7.0) * quadratic!(3)  // x3^2 -> x3
            + coeff!(8.0) * quadratic!(1, 2);

        let changed3 = quad_poly2.reduce_binary_power(&binary_ids);
        assert!(changed3);
        assert_abs_diff_eq!(quad_poly2, expected2);

        // Test case 4: No change case - all non-binary variables
        let original_quad3 = quadratic!(4, 4) + coeff!(2.0) * quadratic!(5, 5);
        let mut quad_poly3 = original_quad3.clone();
        let changed4 = quad_poly3.reduce_binary_power(&binary_ids);
        assert!(!changed4); // No change since x4 and x5 are not binary
        assert_abs_diff_eq!(quad_poly3, original_quad3);

        // Test case 5: Empty polynomial
        let mut empty_poly = Quadratic::default();
        let expected_empty = Quadratic::default();
        let changed5 = empty_poly.reduce_binary_power(&binary_ids);
        assert!(!changed5); // No change for empty polynomial
        assert_abs_diff_eq!(empty_poly, expected_empty);
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
