use crate::{
    macros::*,
    v1::{Linear, Monomial, Polynomial, Quadratic},
    MonomialDyn,
};
use approx::AbsDiffEq;
use num::Zero;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    ops::{Add, Mul},
};

use crate::format::format_polynomial;

impl Zero for Polynomial {
    fn zero() -> Self {
        Self { terms: vec![] }
    }

    fn is_zero(&self) -> bool {
        self.terms.iter().all(|term| term.coefficient.is_zero())
    }
}

impl From<f64> for Polynomial {
    fn from(c: f64) -> Self {
        if c.is_zero() {
            return Self::zero();
        }
        Self {
            terms: vec![Monomial {
                ids: vec![],
                coefficient: c,
            }],
        }
    }
}

impl From<Linear> for Polynomial {
    fn from(l: Linear) -> Self {
        l.into_iter()
            .map(|(id, c)| (id.into_iter().collect(), c))
            .collect()
    }
}

impl From<Quadratic> for Polynomial {
    fn from(q: Quadratic) -> Self {
        q.into_iter().collect()
    }
}

impl FromIterator<(MonomialDyn, f64)> for Polynomial {
    fn from_iter<I: IntoIterator<Item = (MonomialDyn, f64)>>(iter: I) -> Self {
        let mut terms = BTreeMap::new();
        for (ids, coefficient) in iter {
            let v: &mut f64 = terms.entry(ids.clone()).or_default();
            *v += coefficient;
            if v.abs() <= f64::EPSILON {
                terms.remove(&ids);
            }
        }
        Self {
            terms: terms
                .into_iter()
                .map(|(ids, coefficient)| Monomial {
                    ids: ids.into_inner(),
                    coefficient,
                })
                .collect(),
        }
    }
}

impl<'a> IntoIterator for &'a Polynomial {
    type Item = (MonomialDyn, f64);
    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(
            self.terms
                .iter()
                .map(|term| (MonomialDyn::new(term.ids.clone()), term.coefficient)),
        )
    }
}

impl Polynomial {
    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.terms
            .iter()
            .flat_map(|term| term.ids.iter())
            .cloned()
            .collect()
    }

    pub fn degree(&self) -> u32 {
        self.terms
            .iter()
            .map(|term| term.ids.len() as u32)
            .max()
            .unwrap_or(0)
    }

    pub fn as_linear(self) -> Option<Linear> {
        self.terms
            .into_iter()
            .map(|Monomial { ids, coefficient }| match ids.as_slice() {
                [id] => Some((Some(*id), coefficient)),
                [] => Some((None, coefficient)),
                _ => None,
            })
            .collect::<Option<Linear>>()
    }

    /// Downcast to a constant if the polynomial is a constant.
    pub fn as_constant(self) -> Option<f64> {
        if self.terms.len() >= 2 {
            return None;
        }
        if self.terms.len() == 1 {
            if self.terms[0].ids.is_empty() {
                return Some(self.terms[0].coefficient);
            } else {
                return None;
            }
        }
        Some(0.0)
    }

    pub fn get_constant(&self) -> f64 {
        self.terms
            .iter()
            .filter_map(|m| {
                if m.ids.is_empty() {
                    Some(m.coefficient)
                } else {
                    None
                }
            })
            .next()
            .unwrap_or_default()
    }
}

impl Add for Polynomial {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let mut terms = BTreeMap::new();
        for term in self.terms.iter().chain(rhs.terms.iter()) {
            let value: &mut f64 = terms.entry(term.ids.clone()).or_default();
            *value += term.coefficient;
            if value.abs() <= f64::EPSILON {
                terms.remove(&term.ids);
            }
        }
        Self {
            terms: terms
                .into_iter()
                .map(|(ids, coefficient)| Monomial { ids, coefficient })
                .collect(),
        }
    }
}

impl_add_from!(Polynomial, f64);
impl_add_from!(Polynomial, Linear);
impl_add_from!(Polynomial, Quadratic);
impl_add_inverse!(f64, Polynomial);
impl_add_inverse!(Linear, Polynomial);
impl_add_inverse!(Quadratic, Polynomial);
impl_sub_by_neg_add!(Polynomial, Polynomial);

impl Mul for Polynomial {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        let mut terms = BTreeMap::new();
        for (id_l, value_l) in self.into_iter() {
            for (id_r, value_r) in rhs.clone().into_iter() {
                let ids = id_r * id_l.clone();
                *terms.entry(ids).or_default() += value_l * value_r;
            }
        }
        terms.into_iter().collect()
    }
}

impl Mul<f64> for Polynomial {
    type Output = Self;
    fn mul(mut self, rhs: f64) -> Self {
        if rhs.is_zero() {
            return Self::zero();
        }
        for term in &mut self.terms {
            term.coefficient *= rhs;
        }
        self
    }
}

impl_mul_from!(Polynomial, Linear, Polynomial);
impl_mul_from!(Polynomial, Quadratic, Polynomial);
impl_mul_inverse!(f64, Polynomial);
impl_mul_inverse!(Linear, Polynomial);
impl_mul_inverse!(Quadratic, Polynomial);
impl_neg_by_mul!(Polynomial);

/// Compare coefficients in sup-norm.
impl AbsDiffEq for Polynomial {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        f64::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        if self.terms.len() != other.terms.len() {
            return false;
        }
        let sub = self.clone() - other.clone();
        sub.terms
            .iter()
            .all(|term| term.coefficient.abs() < epsilon)
    }
}

impl fmt::Display for Polynomial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return write!(f, "0");
        }
        format_polynomial(f, self.clone().into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::FunctionParameters;

    test_algebraic!(super::Polynomial);

    #[test]
    fn format() {
        let p = super::Polynomial::from_iter(vec![
            (vec![1, 2, 3].into(), 1.0),
            (vec![2, 3].into(), -1.0),
            (vec![1, 3, 5, 6].into(), 3.0),
        ]);
        assert_eq!(p.to_string(), "3*x1*x3*x5*x6 + x1*x2*x3 - x2*x3");
    }

    proptest! {
        #[test]
        fn test_as_linear(p in super::Polynomial::arbitrary_with(FunctionParameters{ num_terms: 5, max_degree: 1, max_id: 10})) {
            let linear = p.clone().as_linear().unwrap();
            prop_assert_eq!(p, super::Polynomial::from(linear));
        }

        #[test]
        fn test_as_constant(p in super::Polynomial::arbitrary_with(FunctionParameters{ num_terms: 1, max_degree: 0, max_id: 10})) {
            let c = p.clone().as_constant().unwrap();
            prop_assert_eq!(p, super::Polynomial::from(c));
        }
    }
}
