use crate::v1::{Linear, Monomial, Polynomial, Quadratic};
use approx::AbsDiffEq;
use num::Zero;
use proptest::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::{Add, Mul},
};

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
                ids: vec![0],
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
                .map(|(ids, coefficient)| Monomial { ids, coefficient })
                .collect(),
        }
    }
}

impl IntoIterator for Polynomial {
    type Item = (Vec<u64>, f64);
    type IntoIter = Box<dyn Iterator<Item = Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
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
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        let num_terms = 0..10_usize;
        let terms = num_terms.prop_flat_map(|num_terms| {
            proptest::collection::vec(
                (
                    proptest::collection::vec(0..(2 * num_terms as u64), 0..=num_terms),
                    prop_oneof![Just(0.0), -1.0..1.0],
                ),
                num_terms,
            )
        });
        terms.prop_map(|terms| terms.into_iter().collect()).boxed()
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

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_zero(a in any::<Polynomial>()) {
            let z = a.clone() - a;
            prop_assert!(z.is_zero());
        }

        #[test]
        fn test_add_associativity(a in any::<Polynomial>(), b in any::<Polynomial>(), c in any::<Polynomial>()) {
            let left = (a.clone() + b.clone()) + c.clone();
            let right = a + (b + c);
            prop_assert!(left.abs_diff_eq(&right, 1e-10));
        }
    }
}
