use crate::v1::{linear::Term, Linear, Quadratic};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    iter::Sum,
    ops::*,
};

impl Linear {
    pub fn new(terms: impl Iterator<Item = (u64, f64)>, constant: f64) -> Self {
        Self {
            terms: terms
                .map(|(id, coefficient)| Term { id, coefficient })
                .collect(),
            constant,
        }
    }

    pub fn single_term(id: u64, coefficient: f64) -> Self {
        Self {
            terms: vec![Term { id, coefficient }],
            constant: 0.0,
        }
    }

    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.terms.iter().map(|term| term.id).collect()
    }
}

/// Create a linear function with a single term by regarding the input as the id of the term.
///
/// ```rust
/// use ommx::v1::Linear;
/// let linear = Linear::from(3);
/// assert_eq!(linear, Linear::single_term(3, 1.0));
/// ```
impl From<u64> for Linear {
    fn from(id: u64) -> Self {
        Self::single_term(id, 1.0)
    }
}

impl From<f64> for Linear {
    fn from(constant: f64) -> Self {
        Self {
            terms: vec![],
            constant,
        }
    }
}

impl FromIterator<(u64, f64)> for Linear {
    fn from_iter<I: IntoIterator<Item = (u64, f64)>>(iter: I) -> Self {
        Self::new(iter.into_iter(), 0.0)
    }
}

impl FromIterator<(Option<u64>, f64)> for Linear {
    fn from_iter<I: IntoIterator<Item = (Option<u64>, f64)>>(iter: I) -> Self {
        let mut map = BTreeMap::new();
        for (id, coefficient) in iter {
            *map.entry(id).or_default() += coefficient;
        }
        let mut out = Linear::default();
        for (id, coefficient) in map {
            if let Some(id) = id {
                out.terms.push(Term { id, coefficient });
            } else {
                out.constant += coefficient;
            }
        }
        out
    }
}

impl IntoIterator for Linear {
    type Item = (Option<u64>, f64);
    // FIXME: Use impl Trait when it is stable
    type IntoIter = Box<dyn Iterator<Item = Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(
            std::iter::once((None, self.constant)).chain(
                self.terms
                    .into_iter()
                    .map(|term| (Some(term.id), term.coefficient)),
            ),
        )
    }
}

impl Add for Linear {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let mut terms = HashMap::new();
        for term in self.terms.iter().chain(rhs.terms.iter()) {
            *terms.entry(term.id).or_default() += term.coefficient;
        }
        Self {
            terms: terms
                .into_iter()
                .map(|(id, coefficient)| Term { id, coefficient })
                .collect(),
            constant: self.constant + rhs.constant,
        }
    }
}

impl Add<f64> for Linear {
    type Output = Self;

    fn add(self, rhs: f64) -> Self {
        Self {
            terms: self.terms,
            constant: self.constant + rhs,
        }
    }
}

impl_add_inverse!(f64, Linear);

impl Sum for Linear {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Linear::from(0), Add::add)
    }
}

impl Mul<f64> for Linear {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self {
        Self {
            terms: self
                .terms
                .into_iter()
                .map(|term| Term {
                    id: term.id,
                    coefficient: term.coefficient * rhs,
                })
                .collect(),
            constant: self.constant * rhs,
        }
    }
}

impl_mul_inverse!(f64, Linear, Linear);

impl Mul for Linear {
    type Output = Quadratic;

    fn mul(self, rhs: Self) -> Quadratic {
        // Create upper triangular matrix
        let mut terms = BTreeMap::new();
        for a in &self.terms {
            for b in &rhs.terms {
                let (row, col) = if a.id < b.id {
                    (a.id, b.id)
                } else {
                    (b.id, a.id)
                };
                *terms.entry((row, col)).or_default() += a.coefficient * b.coefficient;
            }
        }
        let mut quad: Quadratic = terms.into_iter().collect();
        let c = self.constant;
        quad.linear = Some(self * rhs.constant + c * rhs);
        quad
    }
}
