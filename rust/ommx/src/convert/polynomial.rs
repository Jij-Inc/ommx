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
        Self {
            terms: iter
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