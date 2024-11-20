use crate::v1::{Linear, Monomial, Polynomial, Quadratic};
use approx::AbsDiffEq;
use num::Zero;
use proptest::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    ops::{Add, Mul},
};

use super::format::format_polynomial;

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

impl FromIterator<(Vec<u64>, f64)> for Polynomial {
    fn from_iter<I: IntoIterator<Item = (Vec<u64>, f64)>>(iter: I) -> Self {
        let mut terms = BTreeMap::new();
        for (mut ids, coefficient) in iter {
            ids.sort_unstable();
            let v: &mut f64 = terms.entry(ids.clone()).or_default();
            *v += coefficient;
            if v.abs() <= f64::EPSILON {
                terms.remove(&ids);
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

impl IntoIterator for Polynomial {
    type Item = (Vec<u64>, f64);
    type IntoIter = Box<dyn Iterator<Item = Self::Item>>;

    fn into_iter(mut self) -> Self::IntoIter {
        self.terms.sort_unstable_by(|a, b| {
            if a.ids.len() != b.ids.len() {
                b.ids.len().cmp(&a.ids.len())
            } else {
                b.ids.cmp(&a.ids)
            }
        });
        Box::new(
            self.terms
                .into_iter()
                .map(|term| (term.ids, term.coefficient)),
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
}

impl Add for Polynomial {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let mut terms = BTreeMap::new();
        for term in self.terms.iter().chain(rhs.terms.iter()) {
            *terms.entry(term.ids.clone()).or_default() += term.coefficient;
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
            for (mut id_r, value_r) in rhs.clone().into_iter() {
                id_r.append(&mut id_l.clone());
                id_r.sort_unstable();
                *terms.entry(id_r).or_default() += value_l * value_r;
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

impl Arbitrary for Polynomial {
    type Parameters = (usize, usize, u64);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with((num_terms, max_degree, max_id): Self::Parameters) -> Self::Strategy {
        let terms = proptest::collection::vec(
            (
                proptest::collection::vec(0..=max_id, 0..=max_degree),
                prop_oneof![Just(0.0), -1.0..1.0],
            ),
            num_terms,
        );
        terms.prop_map(|terms| terms.into_iter().collect()).boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (0..10_usize, 0..5_usize, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }
}

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
    test_algebraic!(super::Polynomial);

    #[test]
    fn format() {
        let p = super::Polynomial::from_iter(vec![
            (vec![1, 2, 3], 1.0),
            (vec![2, 3], -1.0),
            (vec![1, 3, 5, 6], 3.0),
        ]);
        assert_eq!(p.to_string(), "3*x1*x3*x5*x6 + x1*x2*x3 - x2*x3");
    }
}
