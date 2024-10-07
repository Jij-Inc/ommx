use crate::v1::{Linear, Monomial, Polynomial, Quadratic};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Add,
};

impl From<f64> for Polynomial {
    fn from(c: f64) -> Self {
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
        let mut poly = Polynomial::default();
        for term in l.terms {
            poly.terms.push(Monomial {
                ids: vec![term.id],
                coefficient: term.coefficient,
            })
        }
        poly.terms.push(Monomial {
            ids: vec![0],
            coefficient: l.constant,
        });
        poly
    }
}

impl From<Quadratic> for Polynomial {
    fn from(q: Quadratic) -> Self {
        assert_eq!(q.columns.len(), q.rows.len());
        assert_eq!(q.columns.len(), q.values.len());
        let n = q.columns.len();
        let mut poly = Polynomial::default();
        for i in 0..n {
            poly.terms.push(Monomial {
                ids: vec![q.columns[i], q.rows[i]],
                coefficient: q.values[i],
            })
        }
        if let Some(linear) = q.linear {
            poly = poly + Self::from(linear);
        }
        poly
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

macro_rules! impl_add_from {
    ($other:ty) => {
        impl Add<$other> for Polynomial {
            type Output = Self;
            fn add(self, rhs: $other) -> Self {
                self + Self::from(rhs)
            }
        }

        impl Add<Polynomial> for $other {
            type Output = Polynomial;
            fn add(self, rhs: Polynomial) -> Polynomial {
                rhs + self
            }
        }
    };
}

impl_add_from!(f64);
impl_add_from!(Linear);
impl_add_from!(Quadratic);
